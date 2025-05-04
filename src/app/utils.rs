//! Utility functions for the application.

use askama::Template;
use axum::{
    http::StatusCode,
    response::Html,
};

/// Get an environment variable or panic if not set.
pub fn get_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| {
        panic!(
            "{var_name} must be set as an environment variable (use an empty string if optional)"
        )
    })
}


/// Utility function to render a template and handle errors.
/// 
/// Returns a tuple of `StatusCode` and `Html` response.
pub fn render<T: Template>(template: T) -> Html<String> {
    match template.render() {
        Ok(content) => Html(content),
        Err(_) => Html("Failed to render the HTML template. Something went terribly wrong. Contact the site administrator.".to_string()),
    }
}