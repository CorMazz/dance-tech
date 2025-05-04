use std::sync::Arc;

use axum::{Router, middleware, routing::get};

use crate::{
    AppState,
    app::views::get_home_page,
    auth::middleware::{check_auth_middleware, require_auth_middleware},
    auth::views::{
        get_google_oauth_callback, get_google_oauth_init_flow, get_login_page, get_logout_page,
        get_signup_page, get_user_dropdown, post_login_form, post_signup_form,
    },
};

use tower_http::services::ServeDir;

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/logout", get(get_logout_page))
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            require_auth_middleware,
        ))
        // Anything above this line will redirect to the login page if the user is not logged in
        .route("/", get(get_home_page))
        .route("/sign-up", get(get_signup_page).post(post_signup_form))
        .route("/login", get(get_login_page).post(post_login_form))
        .route("/private/user-dropdown", get(get_user_dropdown))
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            check_auth_middleware,
        ))
        // Anything above this line checks if the user is logged in and adds an AuthStatus extension to the request
        .route("/auth/google", get(get_google_oauth_init_flow))
        .route("/auth/google/callback", get(get_google_oauth_callback))
        .with_state(app_state)
        .nest_service("/static", ServeDir::new("static/"))
}
