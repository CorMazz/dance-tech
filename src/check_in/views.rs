
use std::sync::Arc;
use crate::app::utils::render;
use crate::app::utils::is_htmx_request;
use crate::AppState;
use askama::Template;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use crate::check_in::models::Product;
use crate::check_in::handlers::create_stripe_checkout_session_handler;
use crate::check_in::handlers::get_successful_checkout_session_handler;

// #######################################################################################################################################################
// check_in.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./check_in_templates/check_in.html", blocks = ["content"])]
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

/// Create a Stripe checkout session 
///
/// We are using the Stripe checkout API. Basically, we send the user over to Stripe's webpage to
/// pay for stuff.
#[tracing::instrument(skip(data, headers))]
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

/// This will never be an HTMX request because it is redirected from Stripe,
/// thus we do not need the content block.
#[derive(Template)]
#[template(path = "./check_in_templates/success.html")] 
pub struct SuccessfulPaymentTemplate {
    payment_successful: bool,
    current_time: String
}

/// Query parameters for a successful Stripe Checkout Session response
///
/// Stripe will add the `session_id` as a query parameter to the `success_url` on their 
/// `CreateCheckoutSession` API.
#[derive(Deserialize, Debug)]
pub struct SuccessfulCheckoutSessionQueryParam {
    pub session_id: String
}

#[tracing::instrument(skip(data, headers))]
/// Stripe redirects to this link upon a successful checkout
pub async fn get_successful_checkout_session(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SuccessfulCheckoutSessionQueryParam>,
) -> impl IntoResponse {
    match get_successful_checkout_session_handler(&data, &params.session_id).await {
        Ok(payment_successful) => {
            let current_time = chrono::Utc::now().format("%b %e, %Y").to_string();
            let template = SuccessfulPaymentTemplate { payment_successful, current_time};
            Html(render(template)).into_response()
        }
        Err(err) => err.into_response(&headers),
    }
}

