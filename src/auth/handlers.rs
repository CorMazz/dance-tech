use crate::app::router::{ROUTES, Routes};
use crate::auth::models::User;
use crate::auth::utils::{
    get_user_by_email, hash_password, login_user, validate_reset_password_token, verify_jwt_token,
};
use crate::{AppState, auth::errors::AuthError};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use askama::Template;
use axum::{
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use lettre::AsyncTransport;
use lettre::Message;
use lettre::message::header::ContentType;
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
    basic::BasicClient,
};
use rand::distr::Alphanumeric;
use rand::{Rng, SeedableRng};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error, instrument};

use redis::AsyncCommands;

use super::middleware::check_auth_utility;
use super::views::RequestPasswordResetForm;

// #######################################################################################################################################################
// Sign Up
// #######################################################################################################################################################

/// Registers a user to the database
///
/// By default, a user has no roles
#[instrument(skip(password, data))]
pub async fn register_user_handler(
    data: Arc<AppState>,
    first_name: String,
    last_name: String,
    email: String,
    password: String,
) -> Result<User, AuthError> {
    if get_user_by_email(&email, &data.db).await?.is_some() {
        return Err(AuthError::DuplicateEmail);
    }

    let hashed_password = hash_password(&password)?;

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
    .map_err(|err| {
        error!(%err, "Error registering new user.");
        AuthError::DatabaseError
    })?;

    get_user_by_email(&email, &data.db)
        .await?
        .ok_or_else(|| {
            error!("Unable to retrieve user information immediately after registering user. Something isn't right here.");
            AuthError::DatabaseError
        })
}

// #######################################################################################################################################################
// Login
// #######################################################################################################################################################

#[instrument(skip(password, data, cookie_jar))]
pub async fn login_user_handler(
    data: Arc<AppState>,
    cookie_jar: CookieJar,
    email: String,
    password: String,
) -> Result<impl IntoResponse, AuthError> {
    let user = get_user_by_email(&email, &data.db)
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

    Ok((jar, Redirect::to(ROUTES.check_in)))
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
            error!(
                "Google OAuth is not configured, unable to continue with the authorization flow."
            );
            Err(AuthError::OAuthError)
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
#[derive(Deserialize, Debug)]
pub struct GoogleAccessTokenPayload {
    pub email_verified: Option<bool>,
    pub email: Option<String>,
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub sub: String,
}

#[instrument(skip(data))]
pub async fn google_oauth_callback_handler(
    data: Arc<AppState>,
    cookie_jar: CookieJar,
    callback_params: GoogleOAuthCallbackParams,
) -> Result<impl IntoResponse, AuthError> {
    let csrf_cookie = cookie_jar
        .get("oauth_csrf")
        .ok_or(AuthError::CSRFTokenMismatch)?;

    let pkce_cookie = cookie_jar.get("oauth_pkce_verifier").ok_or_else(|| {
        error!("Unable to get PKCE cookie.");
        AuthError::OAuthError
    })?;

    if csrf_cookie.value() != callback_params.state {
        return Err(AuthError::CSRFTokenMismatch);
    }

    let config = data.google_oauth_config.as_ref().ok_or_else(|| {
        error!("Google OAuth config is missing.");
        AuthError::OAuthError
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
        .map_err(|err| {
            error!(%err, "There was an issue getting the token.");
            AuthError::OAuthError
        })?;

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
        .map_err(|err| {
            error!(%err, "Error getting user info from google.");
            AuthError::OAuthError
        })?
        .json::<GoogleAccessTokenPayload>()
        .await
        .map_err(|err| {
            error!(%err, "Error getting user info from google.");
            AuthError::OAuthError
        })?;

    let email = user_info.email.as_ref().ok_or_else(|| {
        error!("Email not found in the user info: {:#?}", user_info);
        AuthError::OAuthError
    })?;

    let user = match get_user_by_email(email, &data.db).await? {
        Some(user) => user,
        None => {
            // Only register if Google signup is allowed and all fields exist
            if let (Some(first_name), Some(last_name), Some(email)) =
                (user_info.given_name, user_info.family_name, user_info.email)
                && data.auth_config.allow_sign_up_with_google
            {
                let password: String = rand::rngs::StdRng::from_os_rng()
                    .sample_iter(&Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect();

                register_user_handler(data.clone(), first_name, last_name, email, password).await?
            } else {
                return Err(AuthError::AccountNotFound);
            }
        }
    };

    // Log the user in and get a mutable CookieJar
    let mut jar = login_user(user, &data, cookie_jar).await?;

    // Add the profile picture cookie if it exists
    if let Some(picture_url) = user_info.picture {
        let cookie = Cookie::build(("profile_picture", picture_url))
            .path("/")
            .max_age(time::Duration::minutes(
                data.auth_config.access_token_max_age * 60,
            ))
            .http_only(true)
            .same_site(SameSite::Lax);

        jar = jar.add(cookie);
    }

    // Return response once, no duplication
    Ok((jar, Redirect::to(ROUTES.check_in)).into_response())
}
// #######################################################################################################################################################
// Logout
// #######################################################################################################################################################

#[instrument(skip(data))]
pub async fn logout_handler(
    cookie_jar: CookieJar,
    headers: &axum::http::HeaderMap,
    data: Arc<AppState>,
) -> Result<impl IntoResponse, AuthError> {
    let (_user, access_token_uuid) = check_auth_utility(&cookie_jar, data.clone(), headers).await?;

    let refresh_token = cookie_jar
        .get("refresh_token")
        .map(|cookie| cookie.value().to_string())
        .ok_or(AuthError::NotLoggedIn)?;

    let refresh_token_details = verify_jwt_token(
        data.auth_config.refresh_token_public_key.clone(),
        &refresh_token,
    )
    .map_err(|err| {
        error!(%err, "Error while verifying jwt token.");
        AuthError::FatalInternalServerError
    })?;

    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|err| {
            error!(%err, "Error while getting redis client.");
            AuthError::FatalInternalServerError
        })?;

    redis_client
        .del::<_, ()>(&[
            refresh_token_details.token_uuid.to_string(),
            access_token_uuid.to_string(),
        ])
        .await
        .map_err(|err| {
            error!(%err, "Error while deleting token from redis database.");
            AuthError::FatalInternalServerError
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

#[derive(Template)]
#[template(path = "./auth_templates/reset_password_email.html")]
struct EmailTemplate {
    rts: Routes,
    site_root: String,
    token: String,
}

/// Rate limit requests by email and IP address.
#[instrument(skip(data))]
pub async fn post_request_password_reset_handler(
    form: RequestPasswordResetForm,
    client_ip: String,
    data: Arc<AppState>,
) -> Result<(), AuthError> {
    let Some(smtp_config) = data.smtp_config.as_ref() else {
        return Err(AuthError::NoEmailConfig);
    };
    let email = form.email.trim().to_lowercase();

    let Some(user) = get_user_by_email(&email, &data.db).await? else {
        // To prevent user enumeration, return success anyway
        debug!(%email, "No account with this email found for a password reset request. Returning `Ok(())`.");
        return Ok(());
    };

    // Generate secure random token
    let token: String = rand::rngs::StdRng::from_os_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Store token in Redis with TTL (15 minutes = 900 seconds)
    // Key format: "password_reset:{token}"
    let redis_key = format!("password_reset:{token}");

    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|err| {
            error!(%err, "Error while getting redis client.");
            AuthError::FatalInternalServerError
        })?;

    // Add rate limiting by email address and IP

    let email_key = format!("password_reset_req:{email}");

    // Can't put unit type into redis, so just use bool and ignore it.
    let already_requested: Option<bool> = redis_client.get(&email_key).await.map_err(|err| {
        error!(%err, "Error while checking password reset throttle log in redis.");
        AuthError::FatalInternalServerError
    })?;

    if already_requested.is_some() {
        debug!(%email, "Too many password reset requests. Denying access.");
        return Err(AuthError::TooManyRequests);
    }

    // Mark that this email just requested a reset
    let _: () = redis_client
        .set_ex(email_key, true, 450)
        .await
        .map_err(|err| {
            error!(%err, "Error while setting password reset throttle log in redis.");
            AuthError::FatalInternalServerError
        })?;

    let ip_key = format!("password_reset_ip:{client_ip}");
    let requests: Option<u32> = redis_client.get(&ip_key).await.map_err(|err| {
        error!(%err, "Error while setting password reset ip throttle log in redis.");
        AuthError::FatalInternalServerError
    })?;

    if let Some(count) = requests {
        if count >= 1 {
            debug!(%client_ip, "IP exceeded password reset request limit.");
            return Err(AuthError::TooManyRequests);
        }
        let _: () = redis_client.incr(ip_key, 1).await.map_err(|err| {
            error!(%err, "Error while incrementing password reset ip throttle log in redis.");
            AuthError::FatalInternalServerError
        })?;
    } else {
        let _: () = redis_client.set_ex(ip_key, 1, 60).await.map_err(|err| {
            error!(%err, "Error while initializing password reset ip throttle log in redis.");
            AuthError::FatalInternalServerError
        })?;
    }

    let _redis_result: () = redis_client
        .set_ex(redis_key, user.id.to_string(), 900)
        .await
        .map_err(|err| {
            error!(%err, "Error while setting password reset key in redis.");
            AuthError::FatalInternalServerError
        })?;

    let email_body = EmailTemplate {
        rts: ROUTES,
        token,
        site_root: data.app_config.site_url.to_string(),
    }
    .render()
    .map_err(|err| {
        error!(%err, "Fatal error rendering `EmailTemplate`.");
        AuthError::FatalInternalServerError
    })?;

    let email = Message::builder()
        .from(smtp_config.user_email.clone())
        .to(user.email.parse().map_err(|err| {
            error!(%err, %user.email, "Unable to parse user email.");
            AuthError::InternalServerError(format!(
                "Error: Unable to parse user email \"{}\": {}",
                user.email, err
            ))
        })?)
        .subject("DanceTech Password Reset")
        .header(ContentType::TEXT_HTML)
        .body(email_body)
        .map_err(|err| {
            error!(%err, "Unable to create email message.");
            AuthError::InternalServerError(format!("Error: Unable to create email: {err}"))
        })?;

    smtp_config.mailer.send(email).await.map_err(|err| {
        error!(%err, "Unable to send email.");
        AuthError::FatalInternalServerError
    })?;

    debug!("Password reset email sent to {}", user.email);

    Ok(())
}

#[instrument(skip_all)]
pub async fn post_reset_password_handler(
    token: String,
    new_password: String,
    confirm_password: String,
    data: Arc<AppState>,
) -> Result<(), AuthError> {
    if new_password != confirm_password {
        return Err(AuthError::PasswordsDoNotMatch);
    }

    // For POST, consume the token so it can't be reused
    let user = validate_reset_password_token(&token, &data, true).await?;

    let hashed_password = hash_password(&new_password)?;

    sqlx::query!(
        "UPDATE users SET password = $1 WHERE id = $2",
        hashed_password,
        user.id
    )
    .execute(&data.db)
    .await
    .map_err(|err| {
        error!(%err, "Error updating password in DB.");
        AuthError::FatalInternalServerError
    })?;

    debug!("Reset a user's password.");

    Ok(())
}
