use std::collections::HashSet;
use std::sync::Arc;

use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::Roles;
use crate::check_in::handlers::get_products_from_actor;
use crate::check_in::models::Product;
use crate::exam::models::Test;
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::response::Redirect;
use axum::Extension;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::app::router::ROUTES;
use crate::app::router::Routes;
use crate::app::utils::ErrorTemplate;

// #######################################################################################################################################################
// home.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./app_templates/home.html", blocks = ["content"])]
pub struct HomeTemplate {
    rts: Routes,
}

/// Serve the home page template.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_home_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template: HomeTemplate = HomeTemplate { rts: ROUTES };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}


// #[derive(Template)]
// #[template(path = "./app_templates/admin_dashboard.html", blocks = ["content"])]
// pub struct AdminDashboardTemplate {
//     rts: Routes,
//     tests: Vec<Test>,
//     products: Vec<Product>,
// }
//
// /// The admin dashboard lets admins view details about all available products, and 
// pub async fn get_admin_dashboard(
//     State(data): State<Arc<AppState>>,
//     headers: axum::http::HeaderMap,
//     Extension(auth_status): Extension<AuthStatus>,
// ) -> impl IntoResponse {
//
    // if matches!(auth_status, AuthStatus::Unauthorized(..)) ||
    //    matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin()) 
    // {
    //     return Redirect::to(ROUTES.login).into_response();
    // }
//
//
//
//     let products = match get_products_from_actor(&data).await {
//         Ok(products) => products,
//         Err(err) => return err.into_response(&headers),
//     };
//
//     let template = AdminDashboardTemplate { 
//         rts: ROUTES,
//         tests: data.exam_config.tests.clone(),
//         products
//     };
//
//     if is_htmx_request(&headers) {
//         (StatusCode::OK, Html(render(template.as_content()))).into_response()
//     } else {
//         (StatusCode::OK, Html(render(template))).into_response()
//     }
// }


// #######################################################################################################################################################
// Error 404 Response
// #######################################################################################################################################################

/// Serve the error 404 not found page
pub async fn error_404_page() -> impl IntoResponse {
    let template: ErrorTemplate = ErrorTemplate {
        rts: ROUTES,
        error_message: "404 Requested Path Not Found".to_string(),
    };
    (StatusCode::NOT_FOUND, Html(render(template)))
}
