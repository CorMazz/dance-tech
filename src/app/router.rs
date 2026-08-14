use crate::{
    AppState,
    app::{
        exports::{get_export_exams, get_export_user_roles, get_export_users},
        views::{
            delete_user_roles_widget, error_404_page, get_admin_dashboard, get_contact_page,
            get_home_page, get_privacy_page, get_terms_page, get_user_roles_widget,
            post_user_roles_bulk, post_user_roles_widget,
        },
    },
    auth::{
        middleware::{check_auth_middleware, require_auth_middleware},
        views::{
            get_google_oauth_callback, get_google_oauth_init_flow, get_login_page, get_logout_page,
            get_request_password_reset_page, get_reset_password_page, get_signup_page,
            get_user_dropdown, post_login_form, post_request_password_reset_page,
            post_reset_password_page, post_signup_form,
        },
    },
    check_in::views::{
        get_check_in_page, get_successful_checkout_session, post_create_check_out_session,
        post_update_available_products, post_update_cart,
    },
    exam::views::{
        delete_queue_widget, get_graded_test_page, get_join_queue_widget,
        get_proctor_dashboard_page, get_queue_widget, get_search_tests_widget, get_test_page,
        get_user_autocomplete, get_user_dashboard_page, get_user_info_widget, post_live_grading,
        post_queue_widget, post_test_form,
    },
};
use axum::{
    Router, middleware,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::services::ServeDir;

/// A struct containing all routes for the app, to minimize code duplication.
pub struct Routes {
    pub root: &'static str,

    // Auth routes
    pub sign_up: &'static str,
    pub login: &'static str,
    pub logout: &'static str,
    pub request_password_reset: &'static str,
    pub reset_password: &'static str,
    pub google_oauth_init: &'static str,
    pub google_oauth_callback: &'static str,

    // Check-in Routes
    pub check_in: &'static str,
    pub create_checkout_session: &'static str,
    pub stripe_success_callback: &'static str,
    pub update_products: &'static str,
    pub update_cart: &'static str,

    // Exam Routes
    /// The exam route has a variable in it, so it needs to be dynamically generated. See the
    /// associated method `self.administer_exam()`
    pub administer_exam_root: &'static str,
    pub graded_exam_root: &'static str,
    pub live_grading_root: &'static str,
    pub proctor_dashboard: &'static str,
    pub user_dashboard: &'static str,
    pub queue_widget: &'static str,
    pub join_queue_widget: &'static str,
    /// The div that displays a user's name and info when proctoring a test.
    pub user_info_widget: &'static str,
    pub search_exam_widget: &'static str,

    // Misc Routes
    pub user_dropdown: &'static str,
    pub admin_dashboard: &'static str,
    pub export_users: &'static str,
    pub export_exams: &'static str,
    pub export_user_roles: &'static str,
    pub search_users: &'static str,
    pub user_roles: &'static str,
    pub user_roles_bulk: &'static str,
    pub contact: &'static str,
    pub terms: &'static str,
    pub privacy: &'static str,
}

impl Routes {
    /// Dynamically generate the route to view an exam, since there may be any number of exams to
    /// view.
    pub fn administer_exam(&self, exam_id: &(impl ToString + ?Sized)) -> String {
        format!("{}/{}", self.administer_exam_root, exam_id.to_string())
    }

    /// Prefill the user info for the test
    pub fn administer_exam_for_user(
        &self,
        exam_id: &(impl ToString + ?Sized),
        user_email: &impl ToString,
    ) -> String {
        format!(
            "{}?email={}",
            self.administer_exam(exam_id),
            user_email.to_string()
        )
    }

    pub fn graded_exam(&self, exam_id: &(impl ToString + ?Sized)) -> String {
        format!("{}/{}", self.graded_exam_root, exam_id.to_string())
    }

    pub fn live_grading(&self, exam_id: &(impl ToString + ?Sized)) -> String {
        format!("{}/{}", self.live_grading_root, exam_id.to_string())
    }

    /// Used for post and delete methods on the queue.
    pub fn queue_query(
        &self,
        user_id: &(impl ToString + ?Sized),
        test_id: &(impl ToString + ?Sized),
    ) -> String {
        format!(
            "{}?user_id={}&test_index={}",
            self.queue_widget,
            user_id.to_string(),
            test_id.to_string()
        )
    }

    /// Must be a full URL to ensure that users can click on it from their email.
    pub fn reset_password_query(
        &self,
        site_root: &(impl ToString + ?Sized + std::fmt::Display),
        password_token: &(impl ToString + ?Sized + std::fmt::Display),
    ) -> String {
        let mut root = site_root.to_string();
        let path = self.reset_password.trim_start_matches('/');

        if root.ends_with('/') {
            root.pop(); // remove trailing slash
        }

        format!("{root}/{path}?token={password_token}",)
    }
}

/// This constant will be shared across the application and is the absolute truth for all routes
pub const ROUTES: Routes = Routes {
    root: "/",

    // Auth Routes
    sign_up: "/sign-up",
    login: "/login",
    logout: "/logout",
    request_password_reset: "/request-password-reset",
    reset_password: "/reset-password",
    google_oauth_init: "/auth/google",
    google_oauth_callback: "/auth/google/callback",

    // Check-in Routes
    check_in: "/check-in",
    create_checkout_session: "/create-checkout-session",
    stripe_success_callback: "/stripe/success",
    update_products: "/update-products",
    update_cart: "/update-cart",

    // Exam Routes
    administer_exam_root: "/administer-exam",
    graded_exam_root: "/view-exam",
    live_grading_root: "/widgets/live-grading",
    proctor_dashboard: "/exam-proctor",
    user_dashboard: "/exam-dashboard",
    queue_widget: "/widgets/queue",
    join_queue_widget: "/widgets/join-queue",
    user_info_widget: "/widgets/user-info",
    search_exam_widget: "/widgets/search-exam",

    // Misc Routes
    user_dropdown: "/widgets/user-dropdown",
    admin_dashboard: "/admin-dashboard",
    export_users: "/admin-dashboard/export/users",
    export_exams: "/admin-dashboard/export/exams",
    export_user_roles: "/admin-dashboard/export/user-roles",
    search_users: "/widgets/search-users",
    user_roles: "/widgets/user-roles",
    user_roles_bulk: "/widgets/user-roles/bulk-grant",
    contact: "/contact",
    terms: "/terms",
    privacy: "/privacy",
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    // Anything in here will redirect to the login page if the user is not logged in
    let auth_required = Router::new()
        .route(ROUTES.update_products, post(post_update_available_products))
        .route(ROUTES.proctor_dashboard, get(get_proctor_dashboard_page))
        .route(ROUTES.user_info_widget, get(get_user_info_widget))
        .route(ROUTES.search_users, get(get_user_autocomplete))
        .route(ROUTES.admin_dashboard, get(get_admin_dashboard))
        .route(ROUTES.export_users, get(get_export_users))
        .route(ROUTES.export_exams, get(get_export_exams))
        .route(ROUTES.export_user_roles, get(get_export_user_roles))
        .route(
            ROUTES.user_roles,
            get(get_user_roles_widget)
                .post(post_user_roles_widget)
                .delete(delete_user_roles_widget),
        )
        .route(ROUTES.user_roles_bulk, post(post_user_roles_bulk))
        .route(
            &ROUTES.administer_exam("{test_index}"),
            get(get_test_page).post(post_test_form),
        )
        .route(
            &ROUTES.live_grading("{test_index}"),
            post(post_live_grading),
        );

    // Anything in here will check if the user is signed in and add that info to the request
    let check_auth = Router::new()
        .route(ROUTES.sign_up, get(get_signup_page).post(post_signup_form))
        .route(ROUTES.login, get(get_login_page).post(post_login_form))
        .route(ROUTES.check_in, get(get_check_in_page))
        .route(ROUTES.user_dashboard, get(get_user_dashboard_page))
        .route(
            ROUTES.queue_widget,
            get(get_queue_widget)
                .post(post_queue_widget)
                .delete(delete_queue_widget),
        )
        .route(ROUTES.join_queue_widget, get(get_join_queue_widget))
        .route(ROUTES.search_exam_widget, get(get_search_tests_widget))
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
        .route(&ROUTES.graded_exam("{test_id}"), get(get_graded_test_page))
        .route(ROUTES.contact, get(get_contact_page))
        .route(ROUTES.terms, get(get_terms_page))
        .route(ROUTES.privacy, get(get_privacy_page))
        .route(ROUTES.root, get(get_home_page))
        .route(ROUTES.update_cart, post(post_update_cart))
        .route(
            ROUTES.request_password_reset,
            get(get_request_password_reset_page).post(post_request_password_reset_page),
        )
        .route(
            ROUTES.reset_password,
            get(get_reset_password_page).post(post_reset_password_page),
        )
        .route(ROUTES.logout, get(get_logout_page));

    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    // DO NOT EDIT BELOW THIS LINE UNLESS YOU KNOW WHAT YOU'RE DOING
    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

    let auth_required = auth_required.route_layer(middleware::from_fn_with_state(
        app_state.clone(),
        require_auth_middleware,
    ));

    let check_auth = check_auth.route_layer(middleware::from_fn_with_state(
        app_state.clone(),
        check_auth_middleware,
    ));

    // Do not edit this unless necessary. Add routes to the router subsections above this
    Router::new()
        .merge(auth_required)
        // Anything above this line will redirect to the login page if the user is not logged in
        .merge(check_auth)
        // Anything above this line checks if the user is logged in and adds an AuthStatus extension to the request
        .merge(no_auth)
        .fallback(error_404_page)
        .with_state(app_state)
        .nest_service("/static", ServeDir::new("static/"))
}
