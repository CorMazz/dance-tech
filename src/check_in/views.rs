
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
use axum::response::Redirect;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use tracing::debug;
use tracing::error;

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

#[tracing::instrument(skip(data, headers))]
/// Create a Stripe checkout session 
///
/// We are using the Stripe checkout API. Basically, we send the user over to Stripe's webpage to
/// pay for stuff.
pub async fn post_create_check_out_session(
    Path(requested_product): Path<String>,
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap
) -> Result<impl IntoResponse, axum::http::Response<axum::body::Body>> {
    let product = data.check_in_config.products
        .get(&requested_product)
        .ok_or_else(|| {
            let err = CheckInError::InvalidProduct(requested_product);
            error!("{}", err.to_string());
            err.into_response(&headers)
        })?;
   
    let mut success_url = data.app_config.site_url.clone();
    success_url.set_path(STRIPE_SUCCESS_CALLBACK_PATH);

    let mut cancel_url = data.app_config.site_url.clone();
    cancel_url.set_path(STRIPE_CANCEL_CALLBACK_PATH);

    let mut params = HashMap::new();
    params.insert("success_url".to_string(), success_url.to_string());
    params.insert("cancel_url".to_string(), cancel_url.to_string());
    params.insert("mode".to_string(), "payment".to_string());
    params.insert("line_items[0][price]".to_string(), product.price_id.clone());
    params.insert("line_items[0][quantity]".to_string(), "1".to_string());

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth(data.check_in_config.secret_key.clone(), Some("")) 
        .form(&params) // This encodes it as application/x-www-form-urlencoded as Stripe wants
        .send()
        .await
        .map_err(|err| {
            error!(%err, "An error occurred communicating with the Stripe API when creating a checkout session.");
            CheckInError::InternalServerError(Some(err.to_string())).into_response(&headers)
        })?;


    let status = res.status();
    let body = res.text()
        .await
        .map_err(|err| {
            let message = "An error occurred when decoding the error response body from Stripe";
            error!(%err, message); 

            // Do not expose sensitive data to users.
            CheckInError::InternalServerError(Some(message.to_string())).into_response(&headers)
        })?;
    
    if status.is_success() {
        let session: CheckoutSessionResponse = serde_json::from_str(&body)
            .map_err(|err| {
                let message = "Failed to parse Stripe Checkout Session response JSON"; 
                error!(%err, %body, message); 
                CheckInError::InternalServerError(Some(message.to_string())).into_response(&headers)
            })?;

        debug!(%status, parsed_response = ?session, full_response = %body, "Stripe API Response (Success)");
        Ok(Redirect::to(&session.url).into_response())
    } else {
        error!(%status, %body, "Stripe API returned an error");
        Err(CheckInError::StripeApiError.into_response(&headers))
    }

}
