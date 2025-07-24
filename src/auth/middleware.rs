use crate::auth::models::Roles;
use crate::{
    AppState,
    auth::{errors::AuthError, handlers, models::User},
};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, header},
    middleware::Next,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::CookieJar;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::sync::Arc;
use tracing::{debug, error, instrument};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthorizedUser {
    pub user: User,
    pub access_token_uuid: uuid::Uuid,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AuthStatus {
    Authorized(AuthorizedUser),
    Unauthorized(AuthError),
}

/// This function checks if the user is authorized. This is not to be used directly as middleware.
#[instrument(skip(data, cookie_jar, request_headers))]
pub async fn check_auth_utility(
    cookie_jar: CookieJar,
    data: Arc<AppState>,
    request_headers: &HeaderMap,
) -> Result<AuthorizedUser, AuthError> {
    let access_token = cookie_jar
        .get("access_token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            request_headers
                .get(header::AUTHORIZATION)
                .and_then(|auth_header| auth_header.to_str().ok())
                .and_then(|auth_value| {
                    auth_value
                        .strip_prefix("Bearer ")
                        .map(std::string::ToString::to_string)
                })
        });

    let access_token = access_token.ok_or(AuthError::NotLoggedIn)?;

    let access_token_details = handlers::verify_jwt_token(
        data.auth_config.access_token_public_key.clone(),
        &access_token,
    )
    .map_err(|err| {
        error!(%err, "Error verifying jwt token.");
        AuthError::InternalServerError(format!("You should try logging in again."))
    })?;

    let access_token_uuid = uuid::Uuid::parse_str(&access_token_details.token_uuid.to_string())
        .map_err(|_| AuthError::InvalidToken)?;

    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|err| {
            error!(%err, "Error getting redis client.");
            AuthError::FatalInternalServerError
        })?;

    let redis_token_user_id = redis_client
        .get::<_, String>(access_token_uuid.clone().to_string())
        .await
        .map_err(|_| AuthError::ExpiredSession)?;

    let user_id_uuid =
        uuid::Uuid::parse_str(&redis_token_user_id).map_err(|_| AuthError::ExpiredSession)?;

    let user = sqlx::query_as!(
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
        WHERE id = $1
        "#,
        user_id_uuid
    )
        .fetch_optional(&data.db)
        .await
        .map_err(|err| {
            error!(%err, "Error fetching user from database.");
            AuthError::FatalInternalServerError
        })?;

    let user = user.ok_or(AuthError::InvalidUser)?;

    debug!(%user.id, %user.email, %user.first_name, %user.last_name, ?user.roles);

    Ok(AuthorizedUser {
        user,
        access_token_uuid,
    })
}

/// Inserts the auth status into the request but does not require auth
#[instrument(skip(cookie_jar, data, req, next))]
pub async fn check_auth_middleware(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    match check_auth_utility(cookie_jar, data, req.headers()).await {
        Ok(auth_data) => {
            req.extensions_mut()
                .insert(AuthStatus::Authorized(auth_data));
        }
        Err(auth_error) => {
            req.extensions_mut()
                .insert(AuthStatus::Unauthorized(auth_error));
        }
    }
    next.run(req).await
}

/// Check if the user is authorized and redirect to the login page if not
#[instrument(skip(cookie_jar, data, req, next))]
pub async fn require_auth_middleware(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    match check_auth_utility(cookie_jar, data, req.headers()).await {
        Ok(auth_data) => {
            req.extensions_mut()
                .insert(AuthStatus::Authorized(auth_data));
            next.run(req).await
        }
        Err(e) => match e {
            AuthError::NotLoggedIn | AuthError::ExpiredSession => {
                Redirect::to("/login").into_response()
            }
            _ => e.into_response(req.headers()),
        },
    }
}
