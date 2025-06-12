//! Error types for check-in operations.

use axum::response::IntoResponse;
use thiserror::Error;
use tracing::error;
use crate::app::{router::ROUTES, utils::{is_htmx_request, render, ErrorTemplate}};

#[derive(Debug, Clone, Error)]
pub enum CheckInError {
    #[error("An internal server error occurred: {0:?}")]
    InternalServerError(Option<String>),

    #[error("An error occurred when communicating with the Stripe API")]
    /// We purposefully do not include more information to avoid leaking secrets to users.
    StripeApiError,
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
