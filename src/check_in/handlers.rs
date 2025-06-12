use std::collections::HashMap;

use axum::response::Redirect;
use tracing::{debug, error, warn};

use crate::{app::constants::{CHECK_IN_PATH, STRIPE_SUCCESS_CALLBACK_PATH}, check_in::{errors::CheckInError, models::{CheckoutSessionResponse, StripeCheckoutSession, StripePriceList, StripeProductSearchResponse}}, AppState};

use crate::check_in::models::Product;

use super::models::{StripePrice, StripeProduct};

/// Use Stripe's checkout API to direct the user to a payment page.
#[tracing::instrument(skip(data))]
pub async fn create_stripe_checkout_session(
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
    cancel_url.set_path(CHECK_IN_PATH);

    let mut params = HashMap::new();
    params.insert("success_url".to_string(), success_url.to_string());
    params.insert("cancel_url".to_string(), cancel_url.to_string());
    params.insert("mode".to_string(), "payment".to_string());
    params.insert("line_items[0][price]".to_string(), product.price_id.clone());
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

/// Using the session_id from Stripe, confirm that a checkout session ended in payment.
#[tracing::instrument(skip(data))]
pub async fn verify_successful_checkout_session(
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


/// Use the Stripe search API to get all products that are active and have a metadata key of
/// `"show-on-dancetech":"true"`. See the Stripe Dashboard for setting metadata.
#[tracing::instrument(skip(data))]
pub async fn get_stripe_products(
    data: &AppState,
) -> Result<Vec<Product>, CheckInError> {
    let client = &data.http_client;
    let secret_key = &data.check_in_config.secret_key;

    let (product_result, price_result) = tokio::join!(
        fetch_all_products(client, secret_key),
        fetch_all_prices(client, secret_key),
    );
    let all_products = product_result?;
    let all_prices = price_result?;

    let price_map: HashMap<String, &StripePrice> = all_prices
        .iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    let final_products: Vec<Product> = all_products
        .into_iter()
        .filter_map(|p| {
            // Find associated price
            let price_id = p.default_price?;
            let price = price_map.get(&price_id)?;

            // Format the price as currency
            let formatted_price = match price.unit_amount {
                Some(amount) => {
                    #[allow(clippy::cast_precision_loss)]
                    let amount_float = amount as f64 / 100.0;
                    format!("${amount_float:.2}")
                }
                None => return None,
            };

            Some(Product {
                name: p.name,
                id: p.id,
                description: p.description.unwrap_or_default(),
                price: formatted_price,
                price_id,
            })
        })
        .collect();

    Ok(final_products)
}

/// A utility function used to query the Stripe product search API to get all the products that are
/// active and have a `show-on-dancetech: true` metadata tag. This lets us update the products
/// available from the Stripe dashboard rather than from the dance-tech server configuration.
#[tracing::instrument(skip(client, secret_key))]
pub async fn fetch_all_products(client: &reqwest::Client, secret_key: &str) -> Result<Vec<StripeProduct>, CheckInError> {
    let mut all_products = vec![];
    let mut page: Option<String> = None;

    loop {
        let mut req = client
            .get("https://api.stripe.com/v1/products/search")
            .basic_auth(secret_key.to_string(), Some(""));

        if let Some(ref token) = page {
            req = req.query(&[
                ("query", "active:'true' AND metadata['show-on-dancetech']:'true'"),
                ("page", token),
            ]);
        } else {
            req = req.query(&[
                ("query", "active:'true' AND metadata['show-on-dancetech']:'true'")
            ]);
        }

        let res = req.send().await.map_err(|err| {
            error!(%err, "Error querying Stripe for product info");
            CheckInError::InternalServerError(Some("Failed to contact Stripe.".into()))
        })?;

        let status = res.status();
        let body = res.text().await.map_err(|err| {
            error!(%err, "Failed to read Stripe response body");
            CheckInError::InternalServerError(Some("Stripe response body could not be read".into()))
        })?;

        if !status.is_success() {
            error!(%status, %body, "Stripe returned error for product search");
            return Err(CheckInError::StripeApiError);
        }

        let parsed: StripeProductSearchResponse = serde_json::from_str(&body).map_err(|err| {
            error!(%err, %body, "Failed to parse Stripe product response");
            CheckInError::InternalServerError(Some("Stripe response could not be parsed".into()))
        })?;

        debug!(%status, parsed_response = ?parsed.data, full_response = %body, "Received products from Stripe");
        all_products.extend(parsed.data);

        if parsed.has_more {
            if let Some(next) = parsed.next_page {
                page = Some(next);
            } else {
                warn!("has_more was true, but next_page was None — exiting loop");
                break;
            }
        } else {
            break;
        }
    }

    Ok(all_products)
}

/// A utility function used to query the Stripe price search API to get all the prices that are
/// active. This is because the product API only gives us `price_ids` and we want to be able to
/// display an actual dollar amount on the check-in page.
#[tracing::instrument(skip(client, secret_key))]
pub async fn fetch_all_prices(client: &reqwest::Client, secret_key: &str) -> Result<Vec<StripePrice>, CheckInError> {
    let mut all_prices = vec![];
    let mut price_page: Option<String> = None;

    loop {
        let mut req = client
            .get("https://api.stripe.com/v1/prices")
            .basic_auth(secret_key.to_string(), Some(""));

        let mut query = vec![
            ("active", "true"),
            ("currency", "usd"),
        ];
        if let Some(ref token) = price_page {
            query.push(("starting_after", token));
        }
        req = req.query(&query);

        let res = req.send().await.map_err(|err| {
            error!(%err, "Error querying Stripe for price info");
            CheckInError::InternalServerError(Some("Failed to contact Stripe.".into()))
        })?;

        let status = res.status();
        let body = res.text().await.map_err(|err| {
            error!(%err, "Failed to read Stripe response body (prices)");
            CheckInError::InternalServerError(Some("Stripe price response body could not be read".into()))
        })?;

        if !status.is_success() {
            error!(%status, %body, "Stripe returned error for price list");
            return Err(CheckInError::StripeApiError);
        }

        let parsed: StripePriceList = serde_json::from_str(&body).map_err(|err| {
            error!(%err, %body, "Failed to parse Stripe price list response");
            CheckInError::InternalServerError(Some("Stripe price response could not be parsed".into()))
        })?;

        debug!(%status, parsed_response = ?parsed.data, full_response = %body, "Received prices from Stripe");
        price_page = parsed.data.last().map(|price| price.id.clone());
        all_prices.extend(parsed.data);

        if !parsed.has_more {
            break;
        }
    }

    Ok(all_prices)
}
