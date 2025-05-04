use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};

// #######################################################################################################################################################
// home.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./primary_templates/home.html")]
pub struct HomeTemplate {}

// Block rendering functionality is currently not implemented in Askama. Instead of using server-side partial rendering,
// I will just use hx-select to grab <div id="primary-content"> that is in my base template
// #[derive(Template)]
// #[template(path = "./primary_templates/home.html", block = "content")]
// pub struct HomeTemplateContent {}

pub async fn get_home_page() -> impl IntoResponse {
    let template: HomeTemplate = HomeTemplate {};

    (StatusCode::OK, Html(template.render().unwrap()))
}



