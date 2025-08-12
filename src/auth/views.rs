// #######################################################################################################################################################
// user_dropdown.html
// #######################################################################################################################################################

use std::net::SocketAddr;
use std::sync::Arc;

use askama::Template;
use axum::extract::{ConnectInfo, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Extension, Form};
use axum_extra::extract::CookieJar;
use reqwest::StatusCode;
use serde::Deserialize;
use crate::auth::utils::validate_reset_password_token;
use crate::AppState;
use crate::app::router::{ROUTES, Routes};
use crate::app::utils::{is_htmx_request, render};
use crate::auth::handlers::{
    google_oauth_callback_handler, google_oauth_init_flow_handler, login_user_handler, logout_handler, post_request_password_reset_handler, post_reset_password_handler, register_user_handler, GoogleOAuthCallbackParams
};
use crate::auth::models::User;
use crate::auth::{errors::AuthError, middleware::AuthStatus};

/// The one absolute truth for html element IDs that are used across multiple templates
#[allow(clippy::struct_field_names)]
pub struct Ids {
    /// The div where the password email success message will be placed by HTMX.
    password_reset_email_success_container: &'static str,
    /// The div where the "passwords do not match" and "password reset" message will be sent for the reset password page
    password_reset_status_container: &'static str,
}

pub const IDS: Ids = Ids {
    password_reset_email_success_container: "password-reset-email-success-container",
    password_reset_status_container: "password-reset-status-container",
};

#[derive(Template)]
#[template(path = "./app_templates/user_dropdown.html")]
pub struct UserDropdownTemplate {
    rts: Routes,
    user: Option<User>,
    is_proctor: bool,
    is_admin: bool,
}

pub async fn get_user_dropdown(Extension(auth_status): Extension<AuthStatus>) -> impl IntoResponse {
    let user = auth_status.ok();
    let is_proctor = user.as_ref().is_some_and(super::models::User::is_proctor);
    let is_admin = user.as_ref().is_some_and(super::models::User::is_admin);

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
#[template(path = "./auth_templates/sign-up.html", blocks = ["content"])]
pub struct SignUpTemplate {
    rts: Routes,
}

pub async fn get_signup_page(
    headers: axum::http::HeaderMap,
) ->impl IntoResponse {
    let template = SignUpTemplate { rts: ROUTES };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

#[derive(Debug, Deserialize)]
pub struct SignUpForm {
    first_name: String,
    last_name: String,
    email: String,
    password: String,
    confirm_password: String,
}

pub async fn post_signup_form(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Form(sign_up): Form<SignUpForm>,
) -> impl IntoResponse {
    // Validate form data
    if sign_up.password != sign_up.confirm_password {
        return AuthError::PasswordsDoNotMatch.into_response(&headers);
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

#[derive(Template)]
#[template(path = "./auth_templates/request_password_reset.html", blocks = ["content"])]
pub struct RequestPasswordResetTemplate {
    rts: Routes,
    ids: Ids,
    /// If the SMPT env vars weren't set, we cannot perform a password reset. 
    email_functionality_active: bool
}

/// The user facing page for requesting a password reset.
pub async fn get_request_password_reset_page(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
) -> impl IntoResponse {
    let template = RequestPasswordResetTemplate { 
        rts: ROUTES,
        ids: IDS,
        email_functionality_active: data.smtp_config.is_some(),
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))

    }
}

#[derive(Debug, Deserialize)]
pub struct RequestPasswordResetForm { pub email: String }

/// Send an email to reset the user's password
pub async fn post_request_password_reset_page(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Form(form): Form<RequestPasswordResetForm>,
) -> impl IntoResponse {
   
    // I anticipate running this behind Cloudflare, so I'm trying to use Cloudflare's header. If
    // that doesn't exist (because we're doing local development or something else), just use the
    // default IP source
    let user_ip = headers
        .get("CF-Connecting-IP")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(addr)
        .to_string();

    match post_request_password_reset_handler(form, user_ip, data).await {
        Ok(()) => {
            (
                StatusCode::OK,
                Html("If the email exists in our system, a reset link has been sent.".to_string()),
            ).into_response()
        }
        Err(e) => e.into_response(&headers)
    }
}


#[derive(Template)]
#[template(path = "./auth_templates/reset_password.html", blocks = ["content"])]
pub struct ResetPasswordTemplate {
    rts: Routes,
    ids: Ids,
    token: String,

}

#[derive(Deserialize)]
pub struct ResetPasswordQuery {
    /// The token generated by the request password reset handler.
    pub token: String
}

/// The user facing page for resetting a password via email.
pub async fn get_reset_password_page(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Query(query): Query<ResetPasswordQuery>,
) -> impl IntoResponse {

    if let Err(e) = validate_reset_password_token(&query.token, &data, false).await {
        return e.into_response(&headers);
    }

    let template = ResetPasswordTemplate { 
        rts: ROUTES,
        ids: IDS,
        token: query.token,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()

    }
}

#[derive(Deserialize)]
pub struct PostResetPasswordForm {
    pub token: String,
    pub new_password: String,
    pub confirm_password: String,
}

/// If this is not an HTMX request then the response will be ugly but it'll still work.
pub async fn post_reset_password_page(
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
    Form(form): Form<PostResetPasswordForm>,
) -> impl IntoResponse {
    // Check passwords match
    if form.new_password != form.confirm_password {
        return (StatusCode::OK, Html("Password do not match.")).into_response()
    }

    if let Err(e) = post_reset_password_handler(form.token, form.new_password, form.confirm_password, data).await {
        return e.into_response(&headers)
    }

    let success_msg = "Password successfully updated. You can now log in.";
    (StatusCode::OK, Html(success_msg)).into_response()
}


// #######################################################################################################################################################
// Logout Endpoint
// #######################################################################################################################################################

/// Logout the user and return them to the home page
pub async fn get_logout_page(
    cookie_jar: CookieJar,
    headers: axum::http::HeaderMap,
    State(data): State<Arc<AppState>>,
) -> impl IntoResponse {
    match logout_handler(cookie_jar, &headers, data).await {
        Ok(response) => response.into_response(),
        Err(e) => match e {
            AuthError::NotLoggedIn => Redirect::to("/").into_response(),
            _ => e.into_response(&headers),
        },
    }
}
