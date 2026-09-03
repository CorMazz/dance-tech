//! Utility functions for the application.

use crate::app::router::{ROUTES, Routes};
use askama::Template;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};

/// Get an environment variable or panic if not set.
pub fn get_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| {
        panic!(
            "{var_name} must be set as an environment variable (use an empty string if optional)"
        )
    })
}

const RENDER_FAIL: &str = "Failed to render the HTML template. Something went terribly wrong. Contact the site administrator.";

fn html_of(template: &impl Template) -> Html<String> {
    template
        .render()
        .map_or_else(|_| Html(RENDER_FAIL.to_string()), Html)
}

/// Render a template and handle errors.
pub fn render<T: Template>(template: T) -> Html<String> {
    html_of(&template)
}

/// Full page, or the `content` fragment when the request is HTMX.
fn render_fragment_or_full(
    headers: &HeaderMap,
    full: &impl Template,
    fragment: &impl Template,
) -> Html<String> {
    if is_htmx_request(headers) {
        html_of(fragment)
    } else {
        html_of(full)
    }
}

/// `render_fragment_or_full` plus `200 OK`.
pub fn render_page(
    headers: &HeaderMap,
    full: &impl Template,
    fragment: &impl Template,
) -> (StatusCode, Html<String>) {
    (
        StatusCode::OK,
        render_fragment_or_full(headers, full, fragment),
    )
}

/// `render_page` for a template with a `content` block. Evaluates `as_content()`
/// inside a block so it is not a tail-expression temporary.
macro_rules! htmx_page {
    ($headers:expr, $template:ident) => {{
        $crate::app::utils::render_page($headers, &$template, &$template.as_content())
    }};
}
pub(crate) use htmx_page;

/// Like `render_page`, but `HX-Trigger == trigger` renders `triggered` instead.
pub fn render_triggered(
    headers: &HeaderMap,
    trigger: &str,
    full: &impl Template,
    fragment: &impl Template,
    triggered: &impl Template,
) -> (StatusCode, Html<String>) {
    if headers.get("HX-Trigger").is_some_and(|id| id == trigger) {
        (StatusCode::OK, html_of(triggered))
    } else {
        render_page(headers, full, fragment)
    }
}

/// `render_triggered` for `content` plus one other Askama block method.
macro_rules! htmx_triggered {
    ($headers:expr, $trigger:expr, $template:ident, $method:ident) => {{
        $crate::app::utils::render_triggered(
            $headers,
            $trigger,
            &$template,
            &$template.as_content(),
            &$template.$method(),
        )
    }};
}
pub(crate) use htmx_triggered;

/// Shared HTML path for domain errors. HTMX gets the `content` block.
pub fn render_error(headers: &HeaderMap, error_message: impl Into<String>) -> Response {
    let template = ErrorTemplate {
        rts: ROUTES,
        error_message: error_message.into(),
    };
    render_fragment_or_full(headers, &template, &template.as_content()).into_response()
}

/// Check if the request is an HTMX request by looking for the "HX-Request" header.
///
/// The HX-Request header is added by HTMX to indicate that the request is an HTMX request, and it is always `true`, so we just check for its presence.
pub fn is_htmx_request(headers: &HeaderMap) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn htmx_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "true".parse().unwrap());
        headers
    }

    #[test]
    fn is_htmx_request_detects_header() {
        assert!(!is_htmx_request(&HeaderMap::new()));
        assert!(is_htmx_request(&htmx_headers()));
    }

    #[test]
    fn render_page_uses_content_block_for_htmx() {
        let template = ErrorTemplate {
            rts: ROUTES,
            error_message: "boom".into(),
        };
        let full = render_page(&HeaderMap::new(), &template, &template.as_content());
        let frag = render_page(&htmx_headers(), &template, &template.as_content());
        assert!(full.1.0.len() > frag.1.0.len());
        assert!(full.1.0.contains("boom"));
        assert!(frag.1.0.contains("boom"));
    }
}
