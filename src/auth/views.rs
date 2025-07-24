// #######################################################################################################################################################
// user_dropdown.html
// #######################################################################################################################################################

use std::sync::Arc;

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Extension, Form};
use axum_extra::extract::CookieJar;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::AppState;
use crate::app::router::{ROUTES, Routes};
use crate::app::utils::{is_htmx_request, render};
use crate::auth::handlers::{
    GoogleOAuthCallbackParams, google_oauth_callback_handler, google_oauth_init_flow_handler,
    login_user_handler, logout_handler, register_user_handler,
};
use crate::auth::models::User;
use crate::auth::{errors::AuthError, middleware::AuthStatus};

#[derive(Template)]
#[template(path = "./app_templates/user_dropdown.html")]
pub struct UserDropdownTemplate {
    rts: Routes,
    user: Option<User>,
    is_proctor: bool,
    is_admin: bool,
}

pub async fn get_user_dropdown(Extension(auth_status): Extension<AuthStatus>) -> impl IntoResponse {
    let user = match auth_status {
        AuthStatus::Authorized(authorized_user) => Some(authorized_user.user),
        AuthStatus::Unauthorized(_) => None,
    };

    let is_proctor = user.as_ref().is_some_and(|u| u.is_proctor());
    let is_admin = user.as_ref().is_some_and(|u| u.is_admin());

    let template = UserDropdownTemplate {
        rts: ROUTES,
        user,
        is_proctor,
        is_admin,
    };

    (StatusCode::OK, Html(template.render().unwrap()))
}

// #######################################################################################################################################################
// sign-up.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./auth_templates/sign-up.html")]
pub struct SignUpTemplate {
    rts: Routes,
}

pub async fn get_signup_page() -> impl IntoResponse {
    let template = SignUpTemplate { rts: ROUTES };

    (StatusCode::OK, Html(template.render().unwrap()))
}

#[derive(Debug, Deserialize)]
pub struct SignUpForm {
    first_name: String,
    last_name: String,
    email: String,
    password: String,
    confirm_password: String,
}

/// All the errors must return the OK status code for HTMX. Also, they must have an outer element with an id of primary-content
pub async fn post_signup_form(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Form(sign_up): Form<SignUpForm>,
) -> impl IntoResponse {
    // Validate form data
    if sign_up.password != sign_up.confirm_password {
        return (
            StatusCode::OK,
            Html("<h1 id=\"primary-content\">Error: Passwords do not match</h1>"),
        )
            .into_response();
    }

    let user_registered = register_user_handler(
        data,
        sign_up.first_name,
        sign_up.last_name,
        sign_up.email,
        sign_up.password,
    )
    .await;

    match user_registered {
        Ok(()) => Redirect::to("/login").into_response(),
        Err(e) => e.into_response(&headers),
    }
}

// #######################################################################################################################################################
// login.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./auth_templates/login.html", blocks = ["content"])]
pub struct LoginTemplate {
    rts: Routes,
    is_demo_mode: bool,
    google_oauth_enabled: bool,
}

pub async fn get_login_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let template: LoginTemplate = LoginTemplate {
        rts: ROUTES,
        is_demo_mode: data.app_config.is_demo_mode,
        google_oauth_enabled: data.google_oauth_config.is_some(),
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

/// Login form doesn't use HTMX, forcing reload of the navbar (to get the user in the top right)
/// thus, the html can return error status codes and the id of the outer element does not matter (unlike signup)
pub async fn post_login_form(
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Form(login): Form<LoginForm>,
) -> impl IntoResponse {
    match login_user_handler(data, cookie_jar, login.email, login.password).await {
        Ok(response) => response.into_response(),
        Err(e) => e.into_response(&headers),
    }
}

// #######################################################################################################################################################
// Google OAuth Endpoints
// #######################################################################################################################################################

/// Initiate the Google OAuth flow (no view needed, just a redirect)
pub async fn get_google_oauth_init_flow(
    State(data): State<Arc<AppState>>,
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    match google_oauth_init_flow_handler(data, cookie_jar).await {
        Ok(response) => response.into_response(),
        Err(e) => e.into_response(&headers),
    }
}

/// Handle the Google OAuth callback (no view needed, just a redirect)
pub async fn get_google_oauth_callback(
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Query(callback_params): Query<GoogleOAuthCallbackParams>,
) -> impl IntoResponse {
    match google_oauth_callback_handler(data, cookie_jar, callback_params).await {
        Ok(response) => response.into_response(),
        Err(e) => e.into_response(&headers),
    }
}

// #######################################################################################################################################################
// Logout Endpoint
// #######################################################################################################################################################

/// Logout the user and return them to the home page
pub async fn get_logout_page(
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    // To get to this page requires auth so we can expect an authorized user variant of auth status instaed of autherror
    let authorized_user = match auth_status {
        AuthStatus::Authorized(user) => user,
        AuthStatus::Unauthorized(_) => {
            panic!("If this happens, check your auth middleware application.") // TODO: Make this
            // not panic
        }
    };

    match logout_handler(cookie_jar, authorized_user, data).await {
        Ok(response) => response.into_response(),
        Err(e) => match e {
            AuthError::NotLoggedIn => Redirect::to("/").into_response(),
            _ => e.into_response(&headers),
        },
    }
}
