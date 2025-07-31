//! For functions that are used inside of handlers.

use crate::auth::models::Roles;
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use sqlx::types::Json;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{error, instrument};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        errors::AuthError,
        models::{TokenClaims, TokenDetails, User},
    },
};

use redis::{AsyncCommands, RedisError};

pub async fn save_token_data_to_redis(
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

/// Gets a user from the database by email
#[instrument(skip(db))]
pub async fn get_user_by_email(
    email: &str,
    db: &Pool<Postgres>,
) -> Result<Option<User>, AuthError> {
    sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            email, 
            first_name,
            last_name,
            roles as "roles: Json<HashSet<Roles>>",
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
    .map_err(|err| {
        error!(%err, "Error getting user by email.");
        AuthError::DatabaseError
    })
}

/// Gets a user from the database by id
#[instrument(skip(db))]
pub async fn get_user_by_id(id: &Uuid, db: &Pool<Postgres>) -> Result<Option<User>, AuthError> {
    sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            email, 
            first_name,
            last_name,
            roles as "roles: Json<HashSet<Roles>>",
            password, 
            created_at,
            updated_at
        FROM users
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(%err, "Error getting user by id.");
        AuthError::DatabaseError
    })
}

/// Adds the given roles to the user if they don't already have them.
#[instrument(skip(db))]
pub async fn grant_roles(
    user: User,
    roles_to_add: HashSet<Roles>,
    db: &Pool<Postgres>,
) -> Result<(), AuthError> {
    let mut current_roles = user.roles.0;

    let original_len = current_roles.len();

    current_roles.extend(roles_to_add);

    if current_roles.len() == original_len {
        return Ok(());
    }

    // Update in database
    sqlx::query("UPDATE users SET roles = $1, updated_at = $2 WHERE id = $3")
        .bind(Json(current_roles)) // serialize Vec<Roles> back into Json wrapper
        .bind(Utc::now())
        .bind(user.id)
        .execute(db)
        .await
        .map_err(|err| {
            error!(%err, "Error updating user roles.");
            AuthError::DatabaseError
        })?;

    Ok(())
}

/// Searches for a testee by matching the query string to the first name, last name, or email
/// using trigram similarity metrics.
#[instrument(skip(db))]
pub async fn search_for_users(query: String, db: &Pool<Postgres>) -> Result<Vec<User>, AuthError> {
    sqlx::query_as!(
        User,
        r#"
        SELECT 
            id, 
            email, 
            first_name,
            last_name,
            roles as "roles: Json<HashSet<Roles>>",
            password, 
            created_at,
            updated_at
        FROM users
        WHERE first_name % $1
           OR last_name % $1
           OR email % $1
           OR (first_name || ' ' || last_name) % $1
        ORDER BY
           GREATEST(
               similarity(first_name || ' ' || last_name, $1),
               similarity(first_name, $1),
               similarity(last_name, $1),
               similarity(email, $1)
           ) DESC
        LIMIT 5
        "#,
        query
    )
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(%err, "Error searching for users");
        AuthError::DatabaseError
    })
}

#[instrument(skip(data))]
pub async fn login_user(
    user: User,
    data: &Arc<AppState>,
    cookie_jar: CookieJar,
) -> Result<CookieJar, AuthError> {
    let access_token_details = generate_jwt_token(
        user.id,
        data.auth_config.access_token_max_age,
        data.auth_config.access_token_private_key.clone(),
    )
    .map_err(|err| {
        error!(%err, "Error generating jwt token");
        AuthError::FatalInternalServerError
    })?;

    let refresh_token_details = generate_jwt_token(
        user.id,
        data.auth_config.refresh_token_max_age,
        data.auth_config.refresh_token_private_key.clone(),
    )
    .map_err(|err| {
        error!(%err, "Error generating jwt token");
        AuthError::FatalInternalServerError
    })?;

    save_token_data_to_redis(
        data,
        &access_token_details,
        data.auth_config.access_token_max_age,
    )
    .await
    .map_err(|err| {
        error!(%err, "Error while saving token to redis database.");
        AuthError::FatalInternalServerError
    })?;

    save_token_data_to_redis(
        data,
        &refresh_token_details,
        data.auth_config.refresh_token_max_age,
    )
    .await
    .map_err(|err| {
        error!(%err, "Error while saving token to redis database.");
        AuthError::FatalInternalServerError
    })?;

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
