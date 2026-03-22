use crate::app::router::ROUTES;
use crate::check_in::models::{LineItem, Product, ShoppingCart};
use crate::{
    AppState,
    check_in::{
        errors::CheckInError,
        models::{CheckoutSessionResponse, StripeCheckoutSession},
    },
};
use axum::response::Redirect;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::Utc;
use redis::AsyncCommands;
use std::collections::HashMap;
use tracing::{debug, error};
use uuid::Uuid;

/// Get the contents of the shopping cart from the Redis db if the shopping cart cookie was found.
/// Otherwise, create the shoping cart id
pub async fn get_or_create_cart(
    data: &AppState,
    jar: CookieJar,
) -> Result<(CookieJar, Uuid, ShoppingCart), CheckInError> {
    // 1️⃣ Extract or generate cart_id
    let cart_id = match jar.get("cart_id") {
        Some(cookie) => Uuid::parse_str(cookie.value()).map_err(|e| {
            error!("Error deserializing shopping cart cookie id: {e}");
            CheckInError::ShoppingCartError
        })?,
        None => Uuid::new_v4(),
    };

    // 2️⃣ Always reset cookie TTL (sliding expiration)
    let cookie = Cookie::build(("cart_id", cart_id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(1));

    // 3️⃣ Redis connection
    let mut redis = data
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| CheckInError::DatabaseError)?;

    let redis_key = format!("cart:{cart_id}");

    // 4️⃣ Load existing cart JSON or create a new one
    let cart: ShoppingCart = match redis.get::<_, String>(&redis_key).await {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|_| ShoppingCart::new()),
        Err(_) => ShoppingCart::new(),
    };

    Ok((jar.add(cookie), cart_id, cart))
}

/// Update a specific product in the cart.
///
/// A cart-id is stored client-side in a cookie. This cart-id corresponds to a `ShoppingCart` object
/// in the Redis db. If there is no cart-id in the cookie, we create a new one and add the cookie.
/// If there is no `ShoppingCart` that corresponds to cart-id in the database, we clear that cookie
/// and create a new one (to update the TTL). This function will communicate with the
/// `StripeProductActor` and get information about the existing products. The product id must match
/// one of the existing products. Then, we update the shopping cart in the redis db to add the new
/// product, or update the quantity if it already exists. If the quantity reaches 0, we remove the
/// product.
#[tracing::instrument(skip(data, cookie_jar))]
pub async fn update_cart(
    data: &AppState,
    cookie_jar: CookieJar,
    products: Vec<Product>,
    product_id: &str,
    quantity: u64,
) -> Result<(CookieJar, ShoppingCart), CheckInError> {
    let (jar, cart_id, cart) = get_or_create_cart(data, cookie_jar).await?;

    if let Some(product) = products.iter().find(|p| p.id.as_str() == product_id) {
        // 5️⃣ Update cart
        let mut updated_cart = cart;
        updated_cart.add_item(product_id, product.clone(), quantity);
        // 6️⃣ Serialize and save back to Redis
        let cart_json = serde_json::to_string(&updated_cart).map_err(|e| {
            error!("There was an issue serializing the shopping cart: {e}");
            CheckInError::ShoppingCartError
        })?;
        let redis_key = format!("cart:{cart_id}");
        let mut redis = data
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| CheckInError::DatabaseError)?;
        let _: () = redis
            .set_ex(&redis_key, cart_json, 60 * 60 * 24)
            .await
            .map_err(|e| {
                error!("Error saving the shopping cart to redis: {e}");
                CheckInError::DatabaseError
            })?; // 1-day TTL
        return Ok((jar, updated_cart));
    }
    Err(CheckInError::InvalidProductError)
}

/// Use Stripe's checkout API to direct the user to a payment page.
///
/// It looks like stripe doesn't actually need the requested product, just the price.
///
/// I'm leaving it because I'm lazy. I wonder why clippy isn't complaining.
///
/// Perhaps in the future I could have a TOS page tied to each specific product. For the moment,
/// I'm just making it global for the whole application via `env_var`.
#[tracing::instrument(skip(data))]
pub async fn create_stripe_checkout_session(
    data: &AppState,
    shopping_cart: &ShoppingCart,
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

    for (i, (product, quantity)) in shopping_cart.items.values().enumerate() {
        params.insert(format!("line_items[{i}][price]"), product.price_id.clone());
        params.insert(format!("line_items[{i}][quantity]"), quantity.to_string());
    }

    params.insert(
        "consent_collection[terms_of_service]".to_string(),
        "required".to_string(),
    );

    params.insert(
        "name_collection[individual][enabled]".to_string(),
        "true".to_string(),
    );

    params.insert(
        "custom_text[terms_of_service_acceptance][message]".to_string(),
        format!(
            "I agree to the terms of the [Liability Waiver]({})",
            data.app_config.tos_url
        ),
    );

    params.insert("allow_promotion_codes".to_string(), "true".to_string());

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
    pub line_items: Vec<LineItem>,
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

    if session.expires_at < Utc::now() {
        return Err(CheckInError::ExpiredCheckoutSession);
    }

    Ok(CheckoutInfo {
        paid: session.payment_status == "paid",
        line_items: session.line_items.data,
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
