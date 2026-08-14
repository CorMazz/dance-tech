//! Error types for authentication operations.

use crate::app::router::ROUTES;
use crate::app::utils::{ErrorTemplate, is_htmx_request, render};
use axum::response::IntoResponse;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AuthError {
    #[error("This email address is already in use.")]
    DuplicateEmail,

    #[error(
        "There was an issue interacting with the database. Please contact the site administrator"
    )]
    DatabaseError,

    #[error("Invalid email or password.")]
    InvalidEmailOrPassword,

    /// Used on the sign-up page. Could probably do this with just a toast instead but oh well.
    #[error("Passwords do not match.")]
    PasswordsDoNotMatch,

    /// Used on the request password reset page
    #[error("You have requested too many password reset requests. Wait a bit.")]
    TooManyRequests,

    #[error("You must be logged in to access this resource.")]
    NotLoggedIn,

    #[error("You do not have permission to access this resource.")]
    Forbidden,

    #[error("An internal server error occurred. Please contact the site administrator.")]
    FatalInternalServerError,

    /// Display a message to the user to tell them how to possibly fix the error.
    #[error("{0:?}")]
    InternalServerError(String),

    #[error("The provided token is invalid.")]
    InvalidToken,

    #[error("Your session has expired. Please login again.")]
    ExpiredSession,

    #[error("Invalid user.")]
    InvalidUser,

    #[error("CSRF token mismatch.")]
    CSRFTokenMismatch,

    #[allow(clippy::enum_variant_names)]
    #[error("There was an issue with OAuth. Please contact the site administrator.")]
    OAuthError,

    #[error(
        "Email functionality is not configured. Passwords cannot be reset. Contact a site administrator."
    )]
    NoEmailConfig,

    #[error(
        "Account not found. Please create an account on our service first; then, if you used a Google account email, you can sign in with Google."
    )]
    AccountNotFound,

    #[error(
        "This token either expired or was never valid. Please complete the password reset within 15 minutes of requesting it."
    )]
    InvalidOrExpiredToken,
}

impl AuthError {
    /// Render the error into the `ErrorTemplate`.
    ///
    /// If the request is an HTMX request, it will render just the content block.
    pub fn into_response(
        self,
        headers: &axum::http::HeaderMap,
    ) -> axum::http::Response<axum::body::Body> {
        let template = ErrorTemplate {
            error_message: self.to_string(),
            rts: ROUTES,
        };

        if is_htmx_request(headers) {
            render(template.as_content()).into_response()
        } else {
            render(template).into_response()
        }
    }
}
