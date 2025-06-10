use std::collections::HashMap;

use axum::response::Redirect;
use tracing::{debug, error};

use crate::{app::constants::{STRIPE_CANCEL_CALLBACK_PATH, STRIPE_SUCCESS_CALLBACK_PATH}, check_in::{errors::CheckInError, models::{CheckoutSessionResponse, StripeCheckoutSession}}, AppState};

/// Use Stripe's checkout API to direct the user to a payment page.
#[tracing::instrument(skip(data))]
pub async fn create_stripe_checkout_session_handler(
    data: &AppState,
    requested_product: &str,
) -> Result<Redirect, CheckInError> {
    let product = data.check_in_config.products
        .get(requested_product)
        .ok_or_else(|| {
            let err = CheckInError::InvalidProduct(requested_product.to_string());
            error!("{}", err.to_string());
            err
        })?;

    let mut success_url = data.app_config.site_url.clone();
    success_url.set_path(STRIPE_SUCCESS_CALLBACK_PATH);
    success_url.set_query(Some("session_id={CHECKOUT_SESSION_ID}")); 

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
        .form(&params)
        .send()
        .await
        .map_err(|err| {
            error!(%err, "An error occurred communicating with the Stripe API when creating a checkout session.");
            CheckInError::InternalServerError(Some(err.to_string()))
        })?;

    let status = res.status();
    let body = res.text()
        .await
        .map_err(|err| {
            let message = "An error occurred when decoding the error response body from Stripe";
            error!(%err, message);
            CheckInError::InternalServerError(Some(message.to_string()))
        })?;

    if status.is_success() {
        let session: CheckoutSessionResponse = serde_json::from_str(&body)
            .map_err(|err| {
                let message = "Failed to parse Stripe Checkout Session response JSON"; 
                error!(%err, %body, message); 
                CheckInError::InternalServerError(Some(message.to_string()))
            })?;

        debug!(%status, parsed_response = ?session, full_response = %body, "Stripe Create Session API Response (Success)");
        Ok(Redirect::to(&session.url))
    } else {
        error!(%status, %body, "Stripe API returned an error");
        Err(CheckInError::StripeApiError)
    }
}


#[tracing::instrument(skip(data))]
pub async fn get_successful_checkout_session_handler(
    data: &AppState,
    session_id: &str,
) -> Result<bool, CheckInError> {
    let url = format!("https://api.stripe.com/v1/checkout/sessions/{session_id}");

    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .basic_auth(&data.check_in_config.secret_key, Some(""))
        .send()
        .await
        .map_err(|err| {
            error!(%err, "Error querying Stripe for session info");
            CheckInError::InternalServerError(Some("Failed to contact Stripe.".into()))
        })?;

    let status = res.status();
    let body = res.text().await.map_err(|err| {
        error!(%err, "Failed to read Stripe response body");
        CheckInError::InternalServerError(Some("Stripe response body could not be read".into()))
    })?;

    if !status.is_success() {
        error!(%status, %body, "Stripe returned an error for session status lookup");
        return Err(CheckInError::StripeApiError);
    } 

    let session: StripeCheckoutSession = serde_json::from_str(&body).map_err(|err| {
        error!(%err, %body, "Failed to parse Stripe session response");
        CheckInError::InternalServerError(Some("Invalid Stripe response format.".into()))
    })?;
     debug!(%status, parsed_response = ?session, full_response = %body, "Stripe Verify Session API Response (Success)");

    Ok(session.payment_status == "paid")
}
