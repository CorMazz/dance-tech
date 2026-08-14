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
            Self::Authorized(user) => Some(user),
            Self::Unauthorized(_) => None,
        }
    }

    /// Return the `User` or redirect the user to the login page.
    pub fn require_auth(self) -> Result<User, Redirect> {
        match self {
            Self::Authorized(user) => Ok(user),
            Self::Unauthorized(_) => Err(Redirect::to(ROUTES.login)),
        }
    }

    /// Return the user if they are an Admin or Proctor.
    pub fn require_superuser(self) -> Result<User, AuthError> {
        match self {
            Self::Authorized(user) if user.is_superuser() => Ok(user),
            Self::Authorized(_) => Err(AuthError::Forbidden),
            Self::Unauthorized(err) => Err(err),
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
        AuthError::InternalServerError("You should try logging in again.".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Roles;
    use sqlx::types::Json;
    use std::collections::HashSet;

    fn user_with_roles(roles: HashSet<Roles>) -> User {
        User {
            id: Uuid::new_v4(),
            first_name: "Test".into(),
            last_name: "User".into(),
            email: "test@example.com".into(),
            password: "hash".into(),
            roles: Json(roles),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn require_superuser_allows_admin_and_proctor() {
        let admin = user_with_roles(HashSet::from([Roles::Admin]));
        let proctor = user_with_roles(HashSet::from([Roles::Proctor]));
        assert!(AuthStatus::Authorized(admin).require_superuser().is_ok());
        assert!(AuthStatus::Authorized(proctor).require_superuser().is_ok());
    }

    #[test]
    fn require_superuser_rejects_dancer_and_anonymous() {
        let dancer = user_with_roles(HashSet::new());
        assert!(matches!(
            AuthStatus::Authorized(dancer).require_superuser(),
            Err(AuthError::Forbidden)
        ));
        assert!(matches!(
            AuthStatus::Unauthorized(AuthError::NotLoggedIn).require_superuser(),
            Err(AuthError::NotLoggedIn)
        ));
    }
}
