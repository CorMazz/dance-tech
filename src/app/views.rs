use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use crate::app::utils::render; 

// #######################################################################################################################################################
// home.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./primary_templates/home.html", blocks = ["content"])]
pub struct HomeTemplate {}

/// Serve the home page template. 
/// 
/// If the request has the "HX-Request" header, it will return just the primary content block.
pub async fn get_home_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template: HomeTemplate = HomeTemplate {};

    if headers.contains_key("HX-Request") {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        // Render the full template
        (StatusCode::OK, Html(render(template)))
    }
}

