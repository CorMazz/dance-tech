use crate::AppState;
use crate::app::router::ROUTES;
use crate::app::router::Routes;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::Roles;
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
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;

use crate::check_in::handlers::get_products_from_actor;

// #######################################################################################################################################################
// check_in.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./check_in_templates/check_in.html", blocks = ["content"])]
pub struct CheckInTemplate {
    rts: Routes,
    products: Vec<Product>,
}

/// Serve the check in page template.
///
/// Show different check-in options (beginner lesson, social dance only, etc) depending on if the
/// user is signed in and if they have access to a certain level of instruction.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_check_in_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let products = match get_products_from_actor(&data).await {
        Ok(products) => products,
        Err(err) => return err.into_response(&headers),
    };
    let template = CheckInTemplate {
        products,
        rts: ROUTES,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

// #######################################################################################################################################################
// Create Checkout Session
// #######################################################################################################################################################

/// Get the product_id and price_id from the button click on the check-in page
#[derive(Deserialize, Debug)]
pub struct CheckoutSessionForm {
    pub product_id: String,
    pub price_id: String,
}

/// Create a Stripe checkout session
///
/// We are using the Stripe checkout API. Basically, we send the user over to Stripe's webpage to
/// pay for stuff.
#[tracing::instrument(skip(data, headers))]
pub async fn post_create_check_out_session(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Form(session_info): Form<CheckoutSessionForm>,
) -> impl IntoResponse {
    match create_stripe_checkout_session(&data, &session_info.product_id, &session_info.price_id)
        .await
    {
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
    rts: Routes,
    payment_successful: bool,
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
    match verify_successful_checkout_session(&data, &params.session_id).await {
        Ok(payment_successful) => {
            let current_time = chrono::Utc::now().format("%b %e, %Y").to_string();
            let template = SuccessfulPaymentTemplate {
                payment_successful,
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
    match auth_status {
        AuthStatus::Authorized(user) => {
            if !user.user.has_role(Roles::Admin) {
                return Redirect::to(ROUTES.login);
            }
        }
        AuthStatus::Unauthorized(_) => return Redirect::to(ROUTES.login),
    }

    if let Err(err) = data.check_in_config.trigger_update_tx.send(()).await {
        error!(%err, "Unable to trigger an update of the products.");
    }

    Redirect::to(ROUTES.check_in)
}
