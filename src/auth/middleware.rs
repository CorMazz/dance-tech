use crate::app::router::ROUTES;
use crate::auth::utils::get_user_by_id;
use crate::{
    AppState,
    auth::{errors::AuthError, models::User, utils},
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
use std::sync::Arc;
use tracing::{debug, error, instrument};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AuthStatus {
    Authorized(User),
    Unauthorized(AuthError),
}

impl AuthStatus {
    /// Return Some(user) or None, discarding the error value.
    pub fn ok(self) -> Option<User> {
        match self {
            AuthStatus::Authorized(user) => Some(user),
            AuthStatus::Unauthorized(_) => None,
        }
    }

    /// Return the `User` or redirect the user to the login page.
    pub fn require_auth(self) -> Result<User, axum::http::Response<axum::body::Body>> {
        match self {
            AuthStatus::Authorized(user) => Ok(user),
            AuthStatus::Unauthorized(_) => Err(Redirect::to(ROUTES.login).into_response()),
        }
    }
}

/// This function checks if the user is authorized. This is not to be used directly as middleware.
#[instrument(skip(data, cookie_jar, request_headers))]
pub async fn check_auth_utility(
    cookie_jar: &CookieJar,
    data: Arc<AppState>,
    request_headers: &HeaderMap,
) -> Result<(User, Uuid), AuthError> {
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

    let access_token_details = utils::verify_jwt_token(
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

    let user_id =
        uuid::Uuid::parse_str(&redis_token_user_id).map_err(|_| AuthError::ExpiredSession)?;

    let user = get_user_by_id(&user_id, &data.db)
        .await?
        .ok_or(AuthError::InvalidUser)?;

    debug!("Middleware identified user: {user:#?}");

    Ok((user, access_token_uuid))
}

/// Inserts the auth status into the request but does not require auth
#[instrument(skip(cookie_jar, data, req, next))]
pub async fn check_auth_middleware(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    match check_auth_utility(&cookie_jar, data, req.headers()).await {
        Ok(auth_data) => {
            req.extensions_mut()
                .insert(AuthStatus::Authorized(auth_data.0));
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
    match check_auth_utility(&cookie_jar, data, req.headers()).await {
        Ok(auth_data) => {
            req.extensions_mut()
                .insert(AuthStatus::Authorized(auth_data.0));
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
