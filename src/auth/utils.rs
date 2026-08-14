//! For functions that are used inside of handlers.

use crate::auth::models::Roles;
use argon2::PasswordHasher;
use argon2::{
    Argon2,
    password_hash::{SaltString, rand_core::OsRng},
};
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
use tracing::{debug, error, instrument};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        errors::AuthError,
        models::{TokenClaims, TokenDetails, User},
    },
};

use redis::{AsyncCommands, RedisError};

/// Hash a plaintext password using Argon2 with a randomly generated salt.
/// Returns the encoded hash as a String.
///
/// Used to sign-up users and to reset their passwords.
///
/// # Errors
/// Returns `AuthError::FatalInternalServerError` if hashing fails for any reason.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| {
            error!(%err, "Error while hashing password.");
            AuthError::FatalInternalServerError
        })
        .map(|hash| hash.to_string())
}

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
/// Returns the modified User struct
#[instrument(skip(db))]
pub async fn grant_roles(
    user: &mut User,
    roles_to_add: HashSet<Roles>,
    db: &Pool<Postgres>,
) -> Result<(), AuthError> {
    let current_roles = &mut user.roles.0;

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

/// Removes the given roles from the user if they currently have them.
/// Returns the modified User struct
#[instrument(skip(db))]
pub async fn revoke_roles(
    user: &mut User,
    roles_to_revoke: HashSet<Roles>,
    db: &Pool<Postgres>,
) -> Result<(), AuthError> {
    let current_roles = &mut user.roles.0;

    let original_len = current_roles.len();

    current_roles.retain(|role| !roles_to_revoke.contains(role));

    if current_roles.len() == original_len {
        // No change — nothing to do
        return Ok(());
    }

    // Update in database
    sqlx::query("UPDATE users SET roles = $1, updated_at = $2 WHERE id = $3")
        .bind(Json(current_roles))
        .bind(Utc::now())
        .bind(user.id)
        .execute(db)
        .await
        .map_err(|err| {
            error!(%err, "Error removing user roles.");
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

/// Every account, sorted by last name then first name. Includes `password` on the struct;
/// callers must not write that field to exports.
#[instrument(skip(db))]
pub async fn list_all_users(db: &Pool<Postgres>) -> Result<Vec<User>, AuthError> {
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
        ORDER BY last_name, first_name, email
        "#
    )
    .fetch_all(db)
    .await
    .map_err(|err| {
        error!(%err, "Error listing all users");
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

/// Validates a password reset token by checking Redis for a matching user ID
/// and confirming that user exists in the database.
///
/// If `consume` is `true`, the token will be deleted from Redis after validation
/// to prevent reuse. Returns the matching `User` on success.
#[instrument(skip(data))]
pub async fn validate_reset_password_token(
    token: &str,
    data: &Arc<AppState>,
    consume: bool,
) -> Result<User, AuthError> {
    let redis_key = format!("password_reset:{token}");

    // Connect to Redis
    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|err| {
            error!(%err, "Error while getting redis client.");
            AuthError::FatalInternalServerError
        })?;

    // Get the user_id from Redis
    let user_id: Option<String> = redis_client.get(&redis_key).await.map_err(|err| {
        error!(%err, "Error while fetching password reset token from redis.");
        AuthError::FatalInternalServerError
    })?;

    let user_id: Uuid = if let Some(uid) = user_id {
        uid.parse().map_err(|err| {
            error!(%err, "Invalid UUID format found in Redis for password reset token.");
            AuthError::AccountNotFound
        })?
    } else {
        debug!("Password reset token invalid or expired: {token}");
        return Err(AuthError::InvalidOrExpiredToken);
    };

    // Retrieve user from DB
    let user = get_user_by_id(&user_id, &data.db).await?.ok_or_else(|| {
        error!("No user found matching the ID in Redis when resetting a password.");
        AuthError::InvalidUser
    })?;

    debug!("Password reset token validated for {}", user.email);

    // Optionally delete the token
    if consume {
        let _: () = redis_client.del(&redis_key).await.map_err(|err| {
            error!(%err, "Error while deleting password reset token from redis.");
            AuthError::FatalInternalServerError
        })?;
    }

    Ok(user)
}
