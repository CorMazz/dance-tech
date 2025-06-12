//! Error types for check-in operations.

use axum::response::IntoResponse;
use thiserror::Error;
use tracing::error;

use crate::app::utils::{ErrorTemplate, is_htmx_request, render};

#[derive(Debug, Clone, Error)]
pub enum CheckInError {
    #[error("An internal server error occurred: {0:?}")]
    InternalServerError(Option<String>),

    #[error("An error occurred when communicating with the Stripe API")]
    /// We purposefully do not include more information to avoid leaking secrets to users.
    StripeApiError,

    #[error("Invalid product requested: {0:?}")]
    InvalidProduct(String),
}

impl CheckInError {
    #[track_caller]
    /// Render the error into the `ErrorTemplate`.
    ///
    /// If the request is an HTMX request, it will render just the content block.
    pub fn into_response(
        self,
        headers: &axum::http::HeaderMap,
    ) -> axum::http::Response<axum::body::Body> {
        let message = match &self {
            // Self::InternalServerError(details) => {
            //     format!(
            //         "An internal error occurred. {}",
            //         details.as_deref().unwrap_or("")
            //     )
            // }
            _ => self.to_string(),
        };
        let template = ErrorTemplate {
            error_message: message,
        };

        if is_htmx_request(headers) {
            render(template.as_content()).into_response()
        } else {
            render(template).into_response()
        }
    }
}
