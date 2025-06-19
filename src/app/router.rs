use std::sync::Arc;
use axum::{
    Router, middleware,
    routing::{get, post},
};
use crate::{
    app::views::{error_404_page, get_home_page}, auth::{
        middleware::{check_auth_middleware, require_auth_middleware},
        views::{
            get_google_oauth_callback, get_google_oauth_init_flow, get_login_page, get_logout_page,
            get_signup_page, get_user_dropdown, post_login_form, post_signup_form,
        },
    }, check_in::views::{
        get_check_in_page, get_successful_checkout_session, post_create_check_out_session,
        post_update_available_products,
    }, exam::views::{get_graded_test_page, get_proctor_dashboard_page, get_test_page, post_test_form}, AppState
};
use tower_http::services::ServeDir;

/// A struct containing all routes for the app, to minimize code duplication.
pub struct Routes {
    pub root: &'static str,

    // Auth routes
    pub sign_up: &'static str,
    pub login: &'static str,
    pub logout: &'static str,
    pub google_oauth_init: &'static str,
    pub google_oauth_callback: &'static str,
    
    // Check-in Routes
    pub check_in: &'static str,
    pub create_checkout_session: &'static str,
    pub stripe_success_callback: &'static str,
    pub update_products: &'static str,

    // Exam Routes
    pub exam_home: &'static str,
    /// The exam route has a variable in it, so it needs to be dynamically generated. See the
    /// associated method `self.administer_exam()`
    pub administer_exam_root: &'static str,
    pub graded_exam_root: &'static str, 
    pub proctor_dashboard: &'static str,

    // Misc Routes
    pub user_dropdown: &'static str,
    pub admin_dashboard: &'static str,
}

impl Routes {
    /// Dynamically generate the route to view an exam, since there may be any number of exams to
    /// view.
    pub fn administer_exam(&self, exam_id: &(impl ToString + ?Sized)) -> String {
        format!("{}/{}", self.administer_exam_root, exam_id.to_string())
    }

    pub fn graded_exam(&self, exam_id: &(impl ToString + ?Sized)) -> String {
        format!("{}/{}", self.graded_exam_root, exam_id.to_string())
    }
}

/// This constant will be shared across the application and is the absolute truth for all routes
pub const ROUTES: Routes = Routes {
    root: "/",

    // Auth Routes
    sign_up: "/sign-up",
    login: "/login",
    logout: "/logout",
    google_oauth_init: "/auth/google",
    google_oauth_callback:  "/auth/google/callback",
    
    // Check-in Routes
    check_in: "/check-in",
    create_checkout_session: "/create-checkout-session",
    stripe_success_callback: "/stripe/success",
    update_products: "/update-products",

    // Exam Routes
    exam_home: "/exam",
    administer_exam_root: "/administer-exam",
    graded_exam_root: "/view-exam",
    proctor_dashboard: "/exam-proctor",

    // Misc Routes
    user_dropdown: "/private/user-dropdown",
    admin_dashboard: "/admin-dashboard",
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    // Anything in here will redirect to the login page if the user is not logged in
    let auth_required = Router::new()
        .route(ROUTES.logout, get(get_logout_page))
        .route(ROUTES.update_products, post(post_update_available_products)) 
        .route(ROUTES.proctor_dashboard, get(get_proctor_dashboard_page))
        .route(&ROUTES.administer_exam("{test_index}"), get(get_test_page).post(post_test_form));

    // Anything in here will check if the user is signed in and add that info to the request
    let check_auth = Router::new()
        .route(ROUTES.root, get(get_home_page))
        .route(ROUTES.sign_up, get(get_signup_page).post(post_signup_form))
        .route(ROUTES.login, get(get_login_page).post(post_login_form))
        .route(ROUTES.check_in, get(get_check_in_page))
        .route(
            ROUTES.create_checkout_session,
            post(post_create_check_out_session),
        )
        .route(
            ROUTES.stripe_success_callback,
            get(get_successful_checkout_session),
        )
        .route(ROUTES.user_dropdown, get(get_user_dropdown));

    // Anything in here does not even check authentication
    let no_auth = Router::new()
        .route(ROUTES.google_oauth_init, get(get_google_oauth_init_flow))
        .route(ROUTES.google_oauth_callback, get(get_google_oauth_callback))
        .route(&ROUTES.graded_exam("{test_id}"), get(get_graded_test_page));

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
