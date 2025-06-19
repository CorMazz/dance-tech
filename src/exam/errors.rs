//! Error types for exam operations.

use axum::response::IntoResponse;
use thiserror::Error;
use crate::app::router::ROUTES;
use crate::app::utils::{ErrorTemplate, is_htmx_request, render};

#[derive(Debug, Clone, Error)]
pub enum ExamError {
    #[error("An internal server error occurred: {0:?}")]
    InternalServerError(Option<String>),

    #[error("There was an error parsing the test results. Please contact the site administrator.")]
    ParseError,

    #[error("The requested graded test does not exist.")]
    GradedTestNotFound,

    #[error("The requested test does not exist. Please contact the site administrator.")]
    TestIndexError,
}

impl ExamError {
    /// Render the error into the `ErrorTemplate`.
    ///
    /// If the request is an HTMX request, it will render just the content block.
    pub fn into_response(
        self,
        headers: &axum::http::HeaderMap,
    ) -> axum::http::Response<axum::body::Body> {

        let template = ErrorTemplate {
            error_message: self.to_string(),
            rts: ROUTES
        };

        if is_htmx_request(headers) {
            render(template.as_content()).into_response()
        } else {
            render(template).into_response()
        }
    }
}
