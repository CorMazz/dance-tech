use crate::check_in::handlers::get_or_create_cart;
use crate::check_in::handlers::update_cart;
use crate::check_in::models::ShoppingCart;
use crate::AppState;
use crate::app::router::ROUTES;
use crate::app::router::Routes;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::Roles;
use crate::check_in::handlers::CheckoutInfo;
use crate::check_in::handlers::create_stripe_checkout_session;
use crate::check_in::handlers::verify_successful_checkout_session;
use crate::check_in::models::Product;
use askama::Template;
use axum::Extension;
use axum::Form;
use axum::extract::Query;
use axum::extract::State;
use axum::response::Redirect;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;

use crate::check_in::handlers::get_products_from_actor;

/// The one absolute truth for html element IDs that are used across multiple templates
pub struct Ids {
    /// The master container that holds the shopping cart drawer and the button to access the
    /// drawer
    shopping_cart_container: &'static str,
}

pub const IDS: Ids = Ids {
    shopping_cart_container: "shopping-cart-container"
};


// #######################################################################################################################################################
// check_in.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./check_in_templates/check_in.html", blocks = ["content"])]
pub struct CheckInTemplate {
    rts: Routes,
    ids: Ids,
    products: Vec<Product>,
    shopping_cart: ShoppingCart,
    /// If the current user is an admin or not. Admins can see all products
    is_admin: bool,
    /// The roles that the current user has, used to filter out products
    roles: HashSet<Roles>,
    /// Let the template know if it needs to display a "hey you have no products" message
    /// This is because not all users can see all products, and I don't want to leave an
    /// unprivileged user with a blank page.
    something_is_displayed: bool,
}

/// Serve the check in page template.
///
/// Show different check-in options (beginner lesson, social dance only, etc) depending on if the
/// user is signed in and if they have access to a certain level of instruction.
/// Admin users can see all products
/// The template itself does the filtering.
///
/// If the request is an HTMX request, it will return just the content block.
#[instrument(skip(data, headers))]
pub async fn get_check_in_page(
    State(data): State<Arc<AppState>>,
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    let user = auth_status.ok();

    let (is_admin, roles): (bool, HashSet<Roles>) = match user {
        Some(user) => (user.is_admin(), user.roles.0),
        None => (false, HashSet::new()),
    };

    let products = match get_products_from_actor(&data).await {
        Ok(products) => products,
        Err(err) => return err.into_response(&headers),
    };

    let something_is_displayed = products.iter().any(|product| {
        is_admin || product.requires_roles.is_subset(&roles) || product.show_preview
    });

    let (cookie_jar, _, shopping_cart) = match get_or_create_cart(&data, cookie_jar).await {
        Ok(res) => res,
        Err(err) => return err.into_response(&headers),
    };

    debug!("Products: {products:#?}\nUser Roles: {roles:#?}\nIs Admin?: {is_admin:#?}");

    let template = CheckInTemplate {
        products,
        rts: ROUTES,
        ids: IDS,
        shopping_cart,
        is_admin,
        roles,
        something_is_displayed,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, cookie_jar, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, cookie_jar, Html(render(template))).into_response()
    }
}


/// We render the button and the shopping cart together, that way when we call `update-cart` we can
/// send the updated cart and swap it with one htmx request.
#[derive(Template)]
#[template(
    ext = "txt",
    source = r#"
{% import "./check_in_templates/macros.html" as macros %}
<div id="shopping-cart-container">
    {% call macros::render_shopping_cart_button(shopping_cart.items.len()) %}
    {% call macros::render_shopping_cart(shopping_cart) %}
</div>
"#
)]
pub struct ShoppingCartTemplate {
    rts: Routes,
    ids: Ids,
    shopping_cart: ShoppingCart
}

/// Get the `product_id` and `price_id` from the button click on the check-in page
#[derive(Deserialize, Debug)]
pub struct UpdateCartForm {
    pub product_id: String,
    pub quantity: u64,
}

#[tracing::instrument(skip(data, headers, cookie_jar))]
pub async fn post_update_cart(
    State(data): State<Arc<AppState>>,
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    Form(update_data): Form<UpdateCartForm>,
) -> impl IntoResponse {
    let products = match get_products_from_actor(&data).await {
        Ok(products) => products,
        Err(err) => return err.into_response(&headers),
    };

    match update_cart(&data, cookie_jar, products, &update_data.product_id, update_data.quantity)
        .await
    {
        Ok((cookie_jar, shopping_cart)) => {
            let template = ShoppingCartTemplate {
                rts: ROUTES,
                ids: IDS,
                shopping_cart,
            };
            (StatusCode::OK, cookie_jar, Html(render(template))).into_response()
        },
        Err(err) => err.into_response(&headers),
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
    State(data): State<Arc<AppState>>,
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let (cookie_jar, _, shopping_cart) = match get_or_create_cart(&data, cookie_jar).await {
        Ok(res) => res,
        Err(err) => return err.into_response(&headers),
    };

    // Clear the cart by just deleting the shopping cart cookie. It'll eventually expire in redis
    match create_stripe_checkout_session(&data, &shopping_cart)
        .await
    {
        Ok(redirect) => (cookie_jar.remove(Cookie::from("cart_id")), redirect).into_response(),
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
    rts: Routes,
    checkout_info: CheckoutInfo,
    current_time: String,
}

/// Query parameters for a successful Stripe Checkout Session response
///
/// Stripe will add the `session_id` as a query parameter to the `success_url` on their
/// `CreateCheckoutSession` API.
#[derive(Deserialize, Debug)]
pub struct SuccessfulCheckoutSessionQueryParam {
    pub session_id: String,
}

/// Stripe redirects to this link upon a successful checkout
#[tracing::instrument(skip(data, headers))]
pub async fn get_successful_checkout_session(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SuccessfulCheckoutSessionQueryParam>,
) -> impl IntoResponse {
    #[allow(clippy::cast_precision_loss)]
    match verify_successful_checkout_session(&data, &params.session_id).await {
        Ok(checkout_info) => {
            let current_time = chrono::Utc::now().format("%b %e, %Y").to_string();
            let template = SuccessfulPaymentTemplate {
                checkout_info,
                current_time,
                rts: ROUTES,
            };
            Html(render(template)).into_response()
        }
        Err(err) => err.into_response(&headers),
    }
}

// #######################################################################################################################################################
// Query Stripe for Products
// #######################################################################################################################################################

/// Query Stripe for available Products
///
/// Used to update the check-in page with the most recent offerings.
#[tracing::instrument(skip(data))]
pub async fn post_update_available_products(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    if let Err(err) = data.check_in_config.trigger_update_tx.send(()).await {
        error!(%err, "Unable to trigger an update of the products.");
    }

    info!("Requesting Stripe Product update.");

    StatusCode::OK.into_response()
}
