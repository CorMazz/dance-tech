
use std::collections::HashMap;
use std::sync::Arc;

use crate::app::constants::STRIPE_CANCEL_CALLBACK_PATH;
use crate::app::constants::STRIPE_SUCCESS_CALLBACK_PATH;
use crate::app::utils::render;
use crate::app::utils::is_htmx_request;
use crate::check_in::models::CheckoutSessionResponse;
use crate::AppState;
use askama::Template;
use axum::extract::Path;
use axum::extract::State;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use crate::check_in::models::Product;
use crate::check_in::handlers::create_stripe_checkout_session_handler;

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

#[tracing::instrument(skip(data, headers))]
/// Create a Stripe checkout session 
///
/// We are using the Stripe checkout API. Basically, we send the user over to Stripe's webpage to
/// pay for stuff.
pub async fn post_create_check_out_session(
    Path(requested_product): Path<String>,
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap
) -> impl IntoResponse {
    match create_stripe_checkout_session_handler(&data,&requested_product).await {
        Ok(redirect) => redirect.into_response(),
        Err(err) => err.into_response(&headers),
    }
}

// #######################################################################################################################################################
// Successful Checkout Session
// #######################################################################################################################################################

#[tracing::instrument(skip(data, headers))]
/// Create a Stripe checkout session 
///
/// We are using the Stripe checkout API. Basically, we send the user over to Stripe's webpage to
/// pay for stuff.
pub async fn successful_check_out_session(
    Path(requested_product): Path<String>,
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap
) -> Result<impl IntoResponse, axum::http::Response<axum::body::Body>> {
    todo!()
}
