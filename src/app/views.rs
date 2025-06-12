use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};

use super::utils::ErrorTemplate;

// #######################################################################################################################################################
// home.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./primary_templates/home.html", blocks = ["content"])]
pub struct HomeTemplate {}

/// Serve the home page template.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_home_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template: HomeTemplate = HomeTemplate {};

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

// #######################################################################################################################################################
// Error 404 Response
// #######################################################################################################################################################

/// Serve the error 404 not found page
pub async fn error_404_page() -> impl IntoResponse {
    let template: ErrorTemplate = ErrorTemplate {
        error_message: "404 Requested Path Not Found".to_string(),
    };
    (StatusCode::NOT_FOUND, Html(render(template)))
}
