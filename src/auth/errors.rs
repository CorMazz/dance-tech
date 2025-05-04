//! Error types for authentication operations.

use askama::Template;
use thiserror::Error;
use axum::{
    response::{Html, IntoResponse},
    http::StatusCode,
};

use crate::app::utils::{is_htmx_request, render};

#[derive(Template)]
#[template(path = "./error_templates/auth_error.html", blocks = ["content"])]
pub struct AuthErrorTemplate { error_message: String }


#[derive(Debug, Clone, Error)]
pub enum AuthError {

    #[error("This email address is already in use.")]
    DuplicateEmail,

    #[error("Invalid email or password.")]
    InvalidEmailOrPassword,

    #[error("You must be logged in to access this resource.")]
    NotLoggedIn,

    #[error("An internal server error occurred: {0:?}")]
    InternalServerError(Option<String>),

    #[error("The provided token is invalid.")]
    InvalidToken,

    #[error("Your session has expired.")]
    ExpiredSession,

    #[error("Invalid user.")]
    InvalidUser,

    #[error("CSRF token mismatch.")]
    CSRFTokenMismatch,

    #[allow(clippy::enum_variant_names)]
    #[error("OAuth error: {0:?}")]
    OAuthError(Option<String>),

    #[error("Account not found. Please create an account on our service first, then if desired you can sign in with your Google account.")]
    AccountNotFound,
}

impl AuthError {
    /// Render the error into the AuthErrorTemplate.
    /// 
    /// If the request is an HTMX request, it will render just the content block.
    pub fn into_response(self, headers: &axum::http::HeaderMap) -> axum::http::Response<axum::body::Body>{
        let message = match &self {
            AuthError::InternalServerError(details) => {
                format!(
                    "An internal error occurred. {}",
                    details.as_deref().unwrap_or("")
                )
            }
            AuthError::OAuthError(details) => format!(
                "There was an OAuth error. {}",
                details.as_deref().unwrap_or("No additional information.")
            ),
            _ => self.to_string(),
        };

        let template = AuthErrorTemplate {
            error_message: message,
        };

        if is_htmx_request(headers) {
            return render(template.as_content()).into_response()
        } else {
            return render(template).into_response()
        }
    }
}