//! Utility functions for the application.

use askama::Template;
use axum::response::Html;
use crate::app::router::Routes;

/// Get an environment variable or panic if not set.
pub fn get_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| {
        panic!(
            "{var_name} must be set as an environment variable (use an empty string if optional)"
        )
    })
}

/// Utility function to render a template and handle errors.
pub fn render<T: Template>(template: T) -> Html<String> {
    template.render().map_or_else(|_| Html("Failed to render the HTML template. Something went terribly wrong. Contact the site administrator.".to_string()),Html)
}

/// Check if the request is an HTMX request by looking for the "HX-Request" header.
///
/// The HX-Request header is added by HTMX to indicate that the request is an HTMX request, and it is always `true`, so we just check for its presence.
pub fn is_htmx_request(headers: &axum::http::HeaderMap) -> bool {
    headers.contains_key("HX-Request")
}

/// The generic template to render all errors for the application.
///
/// The errors themselves implement `IntoResponse` to render into this template.
#[derive(Template)]
#[template(path = "./app_templates/error.html", blocks = ["content"])]
pub struct ErrorTemplate {
    pub rts: Routes,
    pub error_message: String,
}
