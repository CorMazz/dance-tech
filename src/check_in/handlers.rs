use crate::app::router::ROUTES;
use crate::check_in::models::Product;
use crate::{
    AppState,
    check_in::{
        errors::CheckInError,
        models::{CheckoutSessionResponse, StripeCheckoutSession},
    },
};
use axum::response::Redirect;
use std::collections::HashMap;
use tracing::{debug, error};

/// Use Stripe's checkout API to direct the user to a payment page.
#[tracing::instrument(skip(data))]
pub async fn create_stripe_checkout_session(
    data: &AppState,
    requested_product: &str,
    price_id: &str,
) -> Result<Redirect, CheckInError> {
    let mut success_url = data.app_config.site_url.clone();
    success_url.set_path(ROUTES.stripe_success_callback);
    success_url.set_query(Some("session_id={CHECKOUT_SESSION_ID}"));

    let mut cancel_url = data.app_config.site_url.clone();
    cancel_url.set_path(ROUTES.check_in);

    let mut params = HashMap::new();
    params.insert("success_url".to_string(), success_url.to_string());
    params.insert("cancel_url".to_string(), cancel_url.to_string());
    params.insert("mode".to_string(), "payment".to_string());
    params.insert("line_items[0][price]".to_string(), price_id.to_string());
    params.insert("line_items[0][quantity]".to_string(), "1".to_string());

    let client = &data.http_client;
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
    let body = res.text().await.map_err(|err| {
        let message = "An error occurred when decoding the error response body from Stripe";
        error!(%err, message);
        CheckInError::InternalServerError(Some(message.to_string()))
    })?;

    if status.is_success() {
        let session: CheckoutSessionResponse = serde_json::from_str(&body).map_err(|err| {
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

/// Minimal information that we want to return from the `verify_successful_checkout_session` function
#[derive(Debug)]
pub struct CheckoutInfo {
    pub paid: bool,
    pub description: String,
    pub price_cents: u64,
    pub customer_name: String,
}

/// Using the `session_id` from Stripe, confirm that a checkout session ended in payment.
#[tracing::instrument(skip(data))]
pub async fn verify_successful_checkout_session(
    data: &AppState,
    session_id: &str,
) -> Result<CheckoutInfo, CheckInError> {
    let url = format!("https://api.stripe.com/v1/checkout/sessions/{session_id}");

    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .query(&[("expand[]", "line_items")])
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

    // Ensure exactly one line item
    let line_items = &session.line_items.data;
    if line_items.len() != 1 {
        error!(line_items_len = line_items.len(), "Expected exactly one line item in checkout session");
        return Err(CheckInError::StripeApiError);
    }

    let item = &line_items[0];

    Ok(CheckoutInfo {
        paid: session.payment_status == "paid",
        description: item.description.clone(),
        price_cents: item.price.unit_amount,
        customer_name: session.customer_details.name,
    })
}

/// Ping the `ProductManager` actor and request the list of products.
pub async fn get_products_from_actor(data: &AppState) -> Result<Vec<Product>, CheckInError> {
    let (product_tx, product_rx) = tokio::sync::oneshot::channel();

    data.check_in_config
        .product_request_tx
        .send(product_tx)
        .await
        .map_err(|err| {
            error!(%err, "Failed to send request to product manager actor");
            CheckInError::InternalServerError(Some("Failed to request product list".into()))
        })?;

    let products = product_rx.await.map_err(|err| {
        error!(%err, "Product manager actor dropped the response channel");
        CheckInError::InternalServerError(Some("Product manager did not respond".into()))
    })?;

    Ok(products)
}
