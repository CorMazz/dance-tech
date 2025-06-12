//! A module containing actors that will run in separate threads and maintain state that may change
//! as the application runs. I am implementing the actor pattern because it eliminates the
//! potential for lock contention, and it is a shiny new thing that I just learned.
//! Resume-driven-development ftw!

use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tracing::warn;
use crate::check_in::models::StripePriceList;
use crate::check_in::models::StripeProductSearchResponse;
use crate::AppState;
use tokio::{sync::{mpsc, oneshot}, time::{interval, Duration, MissedTickBehavior}};
use crate::check_in::models::Product;
use crate::check_in::errors::CheckInError;
use crate::check_in::models::StripePrice;
use crate::check_in::models::StripeProduct;

/// The main task that will run in a tokio thread to manage the list of products that are offered
/// on the check-in page. Will query Stripe for available products every 5 minutes by default, but
/// can be triggered manually.
///
/// `product_request_rx` is where the axum server will send requests for the current list of products
/// `trigger_update_rx` is where the axum server can send a request to update the current list of
/// products immediately
/// The secret key is from Stripe
///
/// If the clone to send the product request gets too expensive, perhaps use a RwLock instead.
/// In the meantime, FISI
#[instrument(skip(product_request_rx, trigger_update_rx, app_state))]
pub async fn product_manager_actor_runtime(
    mut product_request_rx: mpsc::Receiver<oneshot::Sender<Vec<Product>>>,
    mut trigger_update_rx: mpsc::Receiver<()>,
    app_state: Arc<AppState>,
) {
    info!("Starting ProductManager Actor");
    let mut products: Vec<Product> = vec![];
    let mut periodic_refresh = interval(Duration::from_secs(60 * 10));
    periodic_refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            // Reload every 5 minutes
            _ = periodic_refresh.tick() => {
                match get_stripe_products(&app_state.http_client, &app_state.check_in_config.secret_key).await {
                    Ok(fresh) => {
                        debug!("Auto refresh fetched {} products", fresh.len());
                        products = fresh;
                    }
                    Err(e) => error!(?e, "Auto refresh failed"),
                }
            }

            // Manual trigger of update
            Some(()) = trigger_update_rx.recv() => {
                match get_stripe_products(&app_state.http_client, &app_state.check_in_config.secret_key).await {
                    Ok(fresh) => {
                        debug!("Manual refresh fetched {} products", fresh.len());
                        products = fresh;
                    }
                    Err(e) => error!(?e, "Manual refresh failed"),
                }
            }

            // Product request
            Some(reply_tx) = product_request_rx.recv() => {
                let _ = reply_tx.send(products.clone());
            }

            // Shutdown condition
            else => {
                warn!("No active senders; shutting down actor");
                break;
            }
        }
    }
}


/// Use the Stripe search API to get all products that are active and have a metadata key of
/// `"show-on-dancetech":"true"`. See the Stripe Dashboard for setting metadata.
#[tracing::instrument(skip(client, secret_key))]
pub async fn get_stripe_products(
    client: &reqwest::Client,
    secret_key: &str,
) -> Result<Vec<Product>, CheckInError> {
    let (product_result, price_result) = tokio::join!(
        fetch_all_products(client, secret_key),
        fetch_all_prices(client, secret_key),
    );
    let all_products = product_result?;
    let all_prices = price_result?;

    let price_map: HashMap<String, &StripePrice> =
        all_prices.iter().map(|p| (p.id.clone(), p)).collect();

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
pub async fn fetch_all_products(
    client: &reqwest::Client,
    secret_key: &str,
) -> Result<Vec<StripeProduct>, CheckInError> {
    let mut all_products = vec![];
    let mut page: Option<String> = None;

    loop {
        let mut req = client
            .get("https://api.stripe.com/v1/products/search")
            .basic_auth(secret_key.to_string(), Some(""));

        if let Some(ref token) = page {
            req = req.query(&[
                (
                    "query",
                    "active:'true' AND metadata['show-on-dancetech']:'true'",
                ),
                ("page", token),
            ]);
        } else {
            req = req.query(&[(
                "query",
                "active:'true' AND metadata['show-on-dancetech']:'true'",
            )]);
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
pub async fn fetch_all_prices(
    client: &reqwest::Client,
    secret_key: &str,
) -> Result<Vec<StripePrice>, CheckInError> {
    let mut all_prices = vec![];
    let mut price_page: Option<String> = None;

    loop {
        let mut req = client
            .get("https://api.stripe.com/v1/prices")
            .basic_auth(secret_key.to_string(), Some(""));

        let mut query = vec![("active", "true"), ("currency", "usd")];
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
            CheckInError::InternalServerError(Some(
                "Stripe price response body could not be read".into(),
            ))
        })?;

        if !status.is_success() {
            error!(%status, %body, "Stripe returned error for price list");
            return Err(CheckInError::StripeApiError);
        }

        let parsed: StripePriceList = serde_json::from_str(&body).map_err(|err| {
            error!(%err, %body, "Failed to parse Stripe price list response");
            CheckInError::InternalServerError(Some(
                "Stripe price response could not be parsed".into(),
            ))
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
