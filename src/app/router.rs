use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{
    AppState,
    app::views::{error_404_page, get_home_page},
    auth::{
        middleware::{check_auth_middleware, require_auth_middleware},
        views::{
            get_google_oauth_callback, get_google_oauth_init_flow, get_login_page, get_logout_page,
            get_signup_page, get_user_dropdown, post_login_form, post_signup_form,
        },
    },
    check_in::views::{
        get_check_in_page, get_successful_checkout_session, post_create_check_out_session,
        post_update_available_products,
    },
};

use tower_http::services::ServeDir;

use crate::app::constants::{
    CHECK_IN_PATH, GOOGLE_OAUTH_CALLBACK_PATH, STRIPE_SUCCESS_CALLBACK_PATH,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    // Anything in here will redirect to the login page if the user is not logged in
    let auth_required = Router::new().route("/logout", get(get_logout_page));

    // Anything in here will check if the user is signed in and add that info to the request
    let check_auth = Router::new()
        .route("/", get(get_home_page))
        .route("/sign-up", get(get_signup_page).post(post_signup_form))
        .route("/login", get(get_login_page).post(post_login_form))
        .route(CHECK_IN_PATH, get(get_check_in_page))
        .route(
            "/create-checkout-session/{product}/{price_id}",
            post(post_create_check_out_session),
        )
        .route(
            STRIPE_SUCCESS_CALLBACK_PATH,
            get(get_successful_checkout_session),
        )
        .route("/update-products", get(post_update_available_products)) // TODO: change this to a
        // post request and make it auth required
        .route("/private/user-dropdown", get(get_user_dropdown));

    // Anything in here does not even check authentication
    let no_auth = Router::new()
        .route("/auth/google", get(get_google_oauth_init_flow))
        .route(GOOGLE_OAUTH_CALLBACK_PATH, get(get_google_oauth_callback));

    // Do not edit this unless necessary. Add routes to the router subsections above this
    Router::new()
        .merge(auth_required)
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            require_auth_middleware,
        ))
        // Anything above this line will redirect to the login page if the user is not logged in
        .merge(check_auth)
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            check_auth_middleware,
        ))
        // Anything above this line checks if the user is logged in and adds an AuthStatus extension to the request
        .merge(no_auth)
        .fallback(error_404_page)
        .with_state(app_state)
        .nest_service("/static", ServeDir::new("static/"))
}
