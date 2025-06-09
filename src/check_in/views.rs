
use std::collections::HashMap;
use std::sync::Arc;

use crate::app::utils::render;
use crate::app::utils::is_htmx_request;
use crate::AppState;
use askama::Template;
use axum::extract::Path;
use axum::extract::State;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::check_in::errors::CheckInError;

use crate::check_in::models::Product;

// #######################################################################################################################################################
// check_in.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./primary_templates/check_in.html", blocks = ["content"])]
pub struct CheckInTemplate {
    products: Vec<Product>
}

/// Serve the check in page template.
/// 
/// Show different check-in options (beginner lesson, social dance only, etc) depending on if the
/// user is signed in and if they have access to a certain level of instruction.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_check_in_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap
) -> impl IntoResponse {
    
    let products = data.check_in_config.products
        .values()
        .cloned()
        .collect();

    let template = CheckInTemplate { products };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}


// #######################################################################################################################################################
// Create Checkout Session
// #######################################################################################################################################################

pub async fn post_create_check_out_session(
    Path(requested_product): Path<String>,
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap
) -> Result<impl IntoResponse, axum::http::Response<axum::body::Body>> {
    let product = data.check_in_config.products
        .get(&requested_product)
        .ok_or_else(|| CheckInError::InvalidProduct(requested_product).into_response(&headers))?;
    

    let mut params = HashMap::new();
    params.insert("success_url".to_string(), "localhost/success".to_string());
    params.insert("mode".to_string(), "payment".to_string());
    params.insert("line_items[0][price]".to_string(), product.price.to_string());
    params.insert("line_items[0][quantity]".to_string(), "1".to_string());

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(data.check_in_config.secret_key.clone(), Some("")) // Your secret key
        .form(&params) // This encodes it as application/x-www-form-urlencoded
        .send()
        .await;

    println!("{res:?}");

    Ok(Html("Howdy").into_response())

}
