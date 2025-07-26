//! Error types for exam operations.

use crate::app::router::ROUTES;
use crate::app::utils::{ErrorTemplate, is_htmx_request, render};
use axum::response::IntoResponse;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ExamError {
    #[error("An internal server error occurred. Please contact the site administrator.")]
    FatalInternalServerError,

    /// Display a message to the user to tell them how to possibly fix the error.
    #[error("{0:?}")]
    InternalServerError(String),

    #[error("There was an error with the Redis database. Please contact the site administrator.")]
    RedisError,

    #[error("There was an error with the database. Please contact the site administrator.")]
    DatabaseError,

    #[error("Unable to read file: `{0:?}`")]
    ReadError(String),

    #[error("There was an error parsing the test results. Please contact the site administrator.")]
    ParseError,

    #[error("The requested graded test does not exist.")]
    GradedTestNotFound,

    #[error("The queue is full. Please try again later.")]
    QueueFull,

    #[error("There was an error with the queue. Please contact the site administrator: {0:?}")]
    QueueError(String),

    #[error("This user is already in the queue for this test.")]
    AlreadyInQueue,

    #[error("The requested test does not exist. Please contact the site administrator.")]
    TestIndexError,

    #[error("The given user was not found. Ensure that the dropdown displays user details before submitting a test.")]
    UserNotFound,
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
            rts: ROUTES,
        };

        if is_htmx_request(headers) {
            render(template.as_content()).into_response()
        } else {
            render(template).into_response()
        }
    }
}
