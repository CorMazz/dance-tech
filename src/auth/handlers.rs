use sqlx::types::Json;
use crate::auth::models::Roles;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use base64::{Engine as _, engine::general_purpose};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
    basic::BasicClient,
};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        errors::AuthError,
        middleware::AuthorizedUser,
        models::{TokenClaims, TokenDetails, User},
    },
};

use redis::{AsyncCommands, RedisError};

// #######################################################################################################################################################
// Sign Up
// #######################################################################################################################################################

/// Registers a user to the database
///
/// By default, a user has no roles
pub async fn register_user_handler(
    data: Arc<AppState>,
    first_name: String,
    last_name: String,
    email: String,
    password: String,
) -> Result<(), AuthError> {
    if get_user(&email, &data.db).await?.is_some() {
        return Err(AuthError::DuplicateEmail);
    }

    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            AuthError::InternalServerError(Some(format!("Error while hashing password: {e}")))
        })
        .map(|hash| hash.to_string())?;

    sqlx::query!(
        "INSERT INTO users (first_name,last_name,email,password)
        VALUES ($1, $2, $3, $4)",
        first_name,
        last_name,
        email.to_ascii_lowercase(),
        hashed_password,
    )
    .execute(&data.db)
    .await
    .map_err(|e| AuthError::InternalServerError(Some(format!("Database error: {e}"))))?;
    Ok(())
}

// #######################################################################################################################################################
// Login
// #######################################################################################################################################################

pub async fn login_user_handler(
    data: Arc<AppState>,
    cookie_jar: CookieJar,
    email: String,
    password: String,
) -> Result<impl IntoResponse, AuthError> {
    let user = get_user(&email, &data.db)
        .await?
        .ok_or(AuthError::InvalidEmailOrPassword)?;

    let is_valid = PasswordHash::new(&user.password).is_ok_and(|parsed_hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok_and(|()| true)
    });

    if !is_valid {
        return Err(AuthError::InvalidEmailOrPassword);
    }

    let jar = login_user(user, &data, cookie_jar).await?;

    Ok((jar, Redirect::to("/")))
}

// #######################################################################################################################################################
// Google OAuth Initialize Login Flow Handler
// #######################################################################################################################################################

pub async fn google_oauth_init_flow_handler(
    data: Arc<AppState>,
    cookie_jar: CookieJar,
) -> Result<impl IntoResponse, AuthError> {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    data.google_oauth_config.as_ref().map_or_else(
        || {
            Err(AuthError::InternalServerError(Some(
                "Google OAuth is not configured, unable to continue with the authorization flow."
                    .to_string(),
            )))
        },
        |config| {
            let client = BasicClient::new(config.client_id.clone())
                .set_client_secret(config.client_secret.clone())
                .set_auth_uri(config.auth_uri.clone())
                .set_token_uri(config.token_uri.clone())
                .set_redirect_uri(config.redirect_uri.clone());

            let (auth_url, csrf_token) = client
                .authorize_url(CsrfToken::new_random)
                .add_scope(Scope::new("profile".to_string()))
                .add_scope(Scope::new("email".to_string()))
                .set_pkce_challenge(pkce_challenge)
                .url();

            let csrf_cookie = Cookie::build(("oauth_csrf", csrf_token.secret().clone()))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(true)
                .max_age(time::Duration::minutes(5));

            let pkce_cookie =
                Cookie::build(("oauth_pkce_verifier", pkce_verifier.secret().clone()))
                    .path("/")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(true)
                    .max_age(time::Duration::minutes(5));

            let jar = cookie_jar.add(csrf_cookie).add(pkce_cookie);

            Ok((jar, Redirect::to(auth_url.as_str())))
        },
    )
}

// #######################################################################################################################################################
// Google OAuth Flow Callback Handler
// #######################################################################################################################################################

#[derive(Debug, Deserialize)]
pub struct GoogleOAuthCallbackParams {
    code: String,
    state: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct GoogleAccessTokenPayload {
    pub email_verified: Option<bool>,
    pub email: Option<String>,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub sub: String,
}

pub async fn google_oauth_callback_handler(
    data: Arc<AppState>,
    cookie_jar: CookieJar,
    callback_params: GoogleOAuthCallbackParams,
) -> Result<impl IntoResponse, AuthError> {
    let csrf_cookie = cookie_jar
        .get("oauth_csrf")
        .ok_or(AuthError::CSRFTokenMismatch)?;

    let pkce_cookie = cookie_jar
        .get("oauth_pkce_verifier")
        .ok_or_else(|| AuthError::OAuthError(Some("Unable to get PKCE cookie.".to_string())))?;

    if csrf_cookie.value() != callback_params.state {
        return Err(AuthError::CSRFTokenMismatch);
    }

    let config = data.google_oauth_config.as_ref().ok_or_else(|| {
        error!("Google OAuth config is missing.");
        AuthError::OAuthError(Some("OAuth config not found. That means Google OAuth isn't intended to be accessed. Did you manually type in this endpoint?".to_string()))
    })?;

    let oauth_client = BasicClient::new(config.client_id.clone())
        .set_client_secret(config.client_secret.clone())
        .set_auth_uri(config.auth_uri.clone())
        .set_token_uri(config.token_uri.clone())
        .set_redirect_uri(config.redirect_uri.clone());

    // The following token exchange returns this on success.
    // Ok(
    //     StandardTokenResponse {
    //         access_token: AccessToken([redacted]),
    //         token_type: Bearer,
    //         expires_in: Some(
    //             3599,
    //         ),
    //         refresh_token: None,
    //         scopes: Some(
    //             [
    //                 Scope(
    //                     "https://www.googleapis.com/auth/userinfo.profile",
    //                 ),
    //                 Scope(
    //                     "https://www.googleapis.com/auth/userinfo.email",
    //                 ),
    //                 Scope(
    //                     "openid",
    //                 ),
    //             ],
    //         ),
    //         extra_fields: EmptyExtraTokenFields,
    //     },
    // )
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(callback_params.code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_cookie.value().to_string()))
        .request_async(&data.http_client)
        .await
        .map_err(|e| AuthError::OAuthError(Some(e.to_string())))?;

    // The following request returns this upon success
    // {
    //     "email": String("corrado@mazzarelli.biz"),
    //     "email_verified": Bool(true),
    //     "family_name": String("Mazzarelli"),
    //     "given_name": String("Corrado"),
    //     "name": String("Corrado “Cory” Mazzarelli"),
    //     "picture": String("https://lh3.googleusercontent.com/a/ACg8ocJ-vpjiao2CsPCixOZKm7Oc1U2SecYPxdmhW1hNCL0WaQayvA=s96-c"),
    //     "sub": String("123520143226431893380"),
    // }
    let user_info = data
        .http_client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|e| AuthError::OAuthError(Some(e.to_string())))?
        .json::<GoogleAccessTokenPayload>()
        .await
        .map_err(|e| AuthError::OAuthError(Some(e.to_string())))?;

    let email = user_info.email.as_ref().ok_or_else(|| {
        AuthError::OAuthError(Some("Google did not return an email address when signing in. The app cannot handle this. Try signing in without Google or try again later.".to_string()))
    })?;

    let user = get_user(email, &data.db).await?;

    match user {
        Some(user) => {
            let jar = login_user(user, &data, cookie_jar).await?;
            Ok((jar, Redirect::to("/")).into_response())
        }
        None => Err(AuthError::AccountNotFound),
    }
}
// #######################################################################################################################################################
// Logout
// #######################################################################################################################################################

pub async fn logout_handler(
    cookie_jar: CookieJar,
    authorized_user: AuthorizedUser,
    data: Arc<AppState>,
) -> Result<impl IntoResponse, AuthError> {
    let refresh_token = cookie_jar
        .get("refresh_token")
        .map(|cookie| cookie.value().to_string())
        .ok_or(AuthError::NotLoggedIn)?;

    let refresh_token_details = verify_jwt_token(
        data.auth_config.refresh_token_public_key.clone(),
        &refresh_token,
    )
    .map_err(|e| AuthError::InternalServerError(Some(format!("{e:?}"))))?;

    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e: RedisError| {
            AuthError::InternalServerError(Some(format!("Redis error: {e}")))
        })?;

    redis_client
        .del::<_, ()>(&[
            refresh_token_details.token_uuid.to_string(),
            authorized_user.access_token_uuid.to_string(),
        ])
        .await
        .map_err(|e: RedisError| {
            AuthError::InternalServerError(Some(format!("Redis error: {e}")))
        })?;

    let access_cookie = Cookie::build(("access_token", ""))
        .path("/")
        .max_age(time::Duration::minutes(-1))
        .same_site(SameSite::Lax)
        .http_only(true);
    let refresh_cookie = Cookie::build(("refresh_token", ""))
        .path("/")
        .max_age(time::Duration::minutes(-1))
        .same_site(SameSite::Lax)
        .http_only(true);

    let logged_in_cookie = Cookie::build(("logged_in", "true"))
        .path("/")
        .max_age(time::Duration::minutes(-1))
        .same_site(SameSite::Lax)
        .http_only(false);

    let mut headers = HeaderMap::new();
    headers.append(
        header::SET_COOKIE,
        access_cookie.to_string().parse().unwrap(),
    );
    headers.append(
        header::SET_COOKIE,
        refresh_cookie.to_string().parse().unwrap(),
    );
    headers.append(
        header::SET_COOKIE,
        logged_in_cookie.to_string().parse().unwrap(),
    );

    let mut response = Redirect::to("/").into_response();
    response.headers_mut().extend(headers);
    Ok(response)
}

// #######################################################################################################################################################
// Utility Functions
// #######################################################################################################################################################

async fn save_token_data_to_redis(
    data: &Arc<AppState>,
    token_details: &TokenDetails,
    max_age: i64,
) -> Result<(), RedisError> {
    let mut redis_client = data.redis_client.get_multiplexed_async_connection().await?;
    redis_client
        .set_ex::<_, _, ()>(
            token_details.token_uuid.to_string(),
            token_details.user_id.to_string(),
            #[allow(clippy::cast_sign_loss)]
            {
                (max_age * 60) as u64
            },
        )
        .await?;
    Ok(())
}

/// Gets a user from the database
pub async fn get_user(email: &str, db: &Pool<Postgres>) -> Result<Option<User>, AuthError> {
    sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            email, 
            first_name,
            last_name,
            roles as "roles: Json<Vec<Roles>>",
            password, 
            created_at,
            updated_at
        FROM users
        WHERE email = $1
        "#,
        email.to_ascii_lowercase()
    )
    .fetch_optional(db)
    .await
    .map_err(|e| AuthError::InternalServerError(Some(format!("Database error: {e}"))))
}

async fn login_user(
    user: User,
    data: &Arc<AppState>,
    cookie_jar: CookieJar,
) -> Result<CookieJar, AuthError> {
    let access_token_details = generate_jwt_token(
        user.id,
        data.auth_config.access_token_max_age,
        data.auth_config.access_token_private_key.clone(),
    )
    .map_err(|e: jsonwebtoken::errors::Error| {
        AuthError::InternalServerError(Some(format!("JWT error: {e}")))
    })?;

    let refresh_token_details = generate_jwt_token(
        user.id,
        data.auth_config.refresh_token_max_age,
        data.auth_config.refresh_token_private_key.clone(),
    )
    .map_err(|e: jsonwebtoken::errors::Error| {
        AuthError::InternalServerError(Some(format!("JWT error: {e}")))
    })?;

    save_token_data_to_redis(
        data,
        &access_token_details,
        data.auth_config.access_token_max_age,
    )
    .await
    .map_err(|e: RedisError| AuthError::InternalServerError(Some(format!("Redis error: {e}"))))?;

    save_token_data_to_redis(
        data,
        &refresh_token_details,
        data.auth_config.refresh_token_max_age,
    )
    .await
    .map_err(|e: RedisError| AuthError::InternalServerError(Some(format!("Redis error: {e}"))))?;

    let access_cookie = Cookie::build((
        "access_token",
        access_token_details.token.clone().unwrap_or_default(),
    ))
    .path("/")
    .max_age(time::Duration::minutes(
        data.auth_config.access_token_max_age * 60,
    ))
    .same_site(SameSite::Lax)
    .http_only(true);

    let refresh_cookie = Cookie::build((
        "refresh_token",
        refresh_token_details.token.unwrap_or_default(),
    ))
    .path("/")
    .max_age(time::Duration::minutes(
        data.auth_config.refresh_token_max_age * 60,
    ))
    .same_site(SameSite::Lax)
    .http_only(true);

    let logged_in_cookie = Cookie::build(("logged_in", "true"))
        .path("/")
        .max_age(time::Duration::minutes(
            data.auth_config.access_token_max_age * 60,
        ))
        .same_site(SameSite::Lax)
        .http_only(false);

    Ok(cookie_jar
        .add(access_cookie)
        .add(refresh_cookie)
        .add(logged_in_cookie))
}

pub fn generate_jwt_token(
    user_id: uuid::Uuid,
    ttl: i64,
    private_key: String,
) -> Result<TokenDetails, jsonwebtoken::errors::Error> {
    let bytes_private_key = general_purpose::STANDARD.decode(private_key).unwrap();
    let decoded_private_key = String::from_utf8(bytes_private_key).unwrap();

    let now = chrono::Utc::now();
    let mut token_details = TokenDetails {
        user_id,
        token_uuid: Uuid::new_v4(),
        expires_in: Some((now + chrono::Duration::minutes(ttl)).timestamp()),
        token: None,
    };

    let claims = TokenClaims {
        sub: token_details.user_id.to_string(),
        token_uuid: token_details.token_uuid.to_string(),
        exp: token_details.expires_in.unwrap(),
        iat: now.timestamp(),
        nbf: now.timestamp(),
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_rsa_pem(decoded_private_key.as_bytes())?,
    )?;
    token_details.token = Some(token);
    Ok(token_details)
}

pub fn verify_jwt_token(
    public_key: String,
    token: &str,
) -> Result<TokenDetails, jsonwebtoken::errors::Error> {
    let bytes_public_key = general_purpose::STANDARD.decode(public_key).unwrap();
    let decoded_public_key = String::from_utf8(bytes_public_key).unwrap();

    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);

    let decoded = jsonwebtoken::decode::<TokenClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_rsa_pem(decoded_public_key.as_bytes())?,
        &validation,
    )?;

    let user_id = Uuid::parse_str(decoded.claims.sub.as_str()).unwrap();
    let token_uuid = Uuid::parse_str(decoded.claims.token_uuid.as_str()).unwrap();

    Ok(TokenDetails {
        token: None,
        token_uuid,
        user_id,
        expires_in: None,
    })
}
