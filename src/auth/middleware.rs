use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request},
    middleware::Next,
    response::{IntoResponse, Redirect},
};

use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{handlers, models::User},
    AppState,
};
use redis::AsyncCommands;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthorizedUser {
    pub user: User,
    pub access_token_uuid: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub enum AuthError {
    DuplicateEmail,
    InvalidEmailOrPassword,
    NotLoggedIn,
    InternalServerError(Option<String>),
    InvalidToken,
    ExpiredSession,
    InvalidUser,
    CSRFTokenMismatch,
    #[allow(clippy::enum_variant_names)]
    OAuthError(Option<String>),
    AccountNotFound,
}

// impl AuthError {
//     pub fn status_code(&self) -> StatusCode {
//         match self {
//             AuthError::NotLoggedIn => StatusCode::UNAUTHORIZED,
//             AuthError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
//             AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
//             AuthError::ExpiredSession => StatusCode::UNAUTHORIZED,
//             AuthError::InvalidUser => StatusCode::NOT_FOUND,
//         }
//     }

//     pub fn message(&self) -> &str {
//         match self {
//             AuthError::NotLoggedIn => "Not logged in",
//             AuthError::InternalServerError(Some(ref details)) => details,
//             AuthError::InternalServerError(None) => "Something unexpected went wrong.",
//             AuthError::InvalidToken => "Invalid token",
//             AuthError::ExpiredSession => "Expired session",
//             AuthError::InvalidUser => "User no longer exists",
//         }
//     }
// }

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AuthStatus {
    Authorized(AuthorizedUser),
    Unauthorized(AuthError),
}

/// This function checks if the user is authorized. This is not to be used directly as middleware.
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

    let access_token_details =
        handlers::verify_jwt_token(data.env.access_token_public_key.clone(), &access_token)
            .map_err(|e| AuthError::InternalServerError(Some(format!("{e:?}"))))?;

    let access_token_uuid = uuid::Uuid::parse_str(&access_token_details.token_uuid.to_string())
        .map_err(|_| AuthError::InvalidToken)?;

    let mut redis_client = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AuthError::InternalServerError(Some(format!("Redis error (this shouldn't happen, try again or contact the server administrator): {e}"))))?;

    let redis_token_user_id = redis_client
        .get::<_, String>(access_token_uuid.clone().to_string())
        .await
        .map_err(|_| AuthError::ExpiredSession)?;

    let user_id_uuid =
        uuid::Uuid::parse_str(&redis_token_user_id).map_err(|_| AuthError::ExpiredSession)?;

    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id_uuid)
        .fetch_optional(&data.db)
        .await
        .map_err(|e| AuthError::InternalServerError(Some(format!("Error fetching user from database (this shouldn't happen, try again or contact the server administrator): {e}"))))?;

    let user = user.ok_or(AuthError::InvalidUser)?;

    Ok(AuthorizedUser {
        user,
        access_token_uuid,
    })
}

/// Inserts the auth status into the request
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

/// Redirect to login page if auth failed, otherwise continue
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
        // Purposefully leaving auth_error here so that I can remember to eventually handle the error itself
        Err(auth_error) => Redirect::to("/login").into_response(),
    }
}
