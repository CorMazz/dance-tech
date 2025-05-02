use std::sync::Arc;
use askama::Template;
use axum::{
    extract::{Query, State}, http::StatusCode, response::{Html, IntoResponse, Redirect}, Extension, Form, 
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use crate::{
    auth::{
        handlers::{google_oauth_callback_handler, google_oauth_init_flow_handler, login_user_handler, logout_handler, register_user_handler, GoogleOAuthCallbackParams}, 
        middleware::{AuthError, AuthStatus},
        model::User
    },
    AppState
};

/// A helper function to handle errors consistently
fn error_response(message: &str) -> impl IntoResponse {
    (StatusCode::OK, Html(format!("<h1 id=\"primary-content\">{}</h1>", message))).into_response()
}

// #######################################################################################################################################################
// home.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./primary_templates/home.html")] 
pub struct HomeTemplate { is_demo_mode: bool }

// Block rendering functionality is currently not implemented in Askama. Instead of using server-side partial rendering,
// I will just use hx-select to grab <div id="primary-content"> that is in my base template
// #[derive(Template)]
// #[template(path = "./primary_templates/home.html", block = "content")] 
// pub struct HomeTemplateContent {}

pub async fn get_home_page(    State(data): State<Arc<AppState>>) -> impl IntoResponse  {
    let template: HomeTemplate = HomeTemplate { is_demo_mode: data.env.is_demo_mode };

    (StatusCode::OK, Html(template.render().unwrap()))
}


// #######################################################################################################################################################
// user_dropdown.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./partial_templates/user_dropdown.html")] 
pub struct UserDropdownTemplate {
    user: Option<User>
}

pub async fn get_user_dropdown(
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    
    let user = match auth_status {
        AuthStatus::Authorized(authorized_user) => Some(authorized_user.user),
        AuthStatus::Unauthorized(_) => None
    };

    let template = UserDropdownTemplate { user };

    (StatusCode::OK, Html(template.render().unwrap()))
}

// #######################################################################################################################################################
// sign-up.html
// #######################################################################################################################################################

#[derive(Template)]
#[template(path = "./auth_templates/sign-up.html")] 
pub struct SignUpTemplate {
}

pub async fn get_signup_page() -> impl IntoResponse {
    let template = SignUpTemplate {};

    (StatusCode::OK, Html(template.render().unwrap()))
}

#[derive(Debug, Deserialize)]
pub struct SignUpForm {
    first_name: String,
    last_name: String,
    email: String,
    password: String,
    confirm_password: String,
    licensing_key: String,
}

/// All the errors must return the OK status code for HTMX. Also, they must have an outer element with an id of primary-content
pub async fn post_signup_form(
    State(data): State<Arc<AppState>>,
    Form(sign_up) : Form<SignUpForm>,
) -> impl IntoResponse {

    // Validate form data
    if sign_up.password != sign_up.confirm_password {
        return (
            StatusCode::OK,
            Html("<h1 id=\"primary-content\">Error: Passwords do not match</h1>"),
        ).into_response();
    }

    let user_registered = register_user_handler(data, sign_up.first_name, sign_up.last_name, sign_up.email, sign_up.password, sign_up.licensing_key).await;

    match user_registered {
        Ok(_) => return Redirect::to("/login").into_response(),
        Err(e) => match e {
            AuthError::DuplicateEmail => return (StatusCode::OK, Html("<h1 id=\"primary-content\">Error: Duplicate Email</h1>")).into_response(),
            AuthError::InvalidLicensingKey => return (StatusCode::OK, Html("<h1 id=\"primary-content\">Error: Invalid Licensing Key</h1>")).into_response(),
            AuthError::InternalServerError(ee) => return (StatusCode::OK, Html(format!("<h1 id=\"primary-content\">Error: {:?}</h1>", ee))).into_response(),
            _ => return (StatusCode::INTERNAL_SERVER_ERROR, Html("<h1 id=\"primary-content\">Unexpected error occurred, this should be impossible.</h1>")).into_response() // This should never happen
        }
    }
}


// #######################################################################################################################################################
// login.html
// #######################################################################################################################################################


#[derive(Template)]
#[template(path = "./auth_templates/login.html")] 
pub struct LoginTemplate {
    is_demo_mode: bool,
    google_oauth_enabled: bool,
}

pub async fn get_login_page(State(data): State<Arc<AppState>>) -> impl IntoResponse  {
    let template: LoginTemplate = LoginTemplate {is_demo_mode: data.env.is_demo_mode, google_oauth_enabled: data.google_oauth_config.is_some()};

    (StatusCode::OK, Html(template.render().unwrap()))
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}

/// Login form doesn't use HTMX to force reload of the navbar (to get the user in the top right)
/// so the html can return error status codes and the id of the outer element does not matter (unlike signup)
pub async fn post_login_form(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    Form(login) : Form<LoginForm>,
) -> impl IntoResponse {

    match login_user_handler(data, cookie_jar, login.email, login.password).await {
        Ok(response) => return response.into_response(),
        Err(e) => match e {
            AuthError::InvalidEmailOrPassword => return (StatusCode::OK, Html("<h1>Invalid Email or Password</h1>")).into_response(),
            AuthError::InternalServerError(ee) => return (StatusCode::OK, Html(format!("Error: {:?}", ee))).into_response(),
            _ => return (StatusCode::OK, Html("<h1>Error: Unexpected error occurred</h1>")).into_response()
        }
    }
}

pub async fn get_google_oauth_init_flow(
    State(data): State<Arc<AppState>>,
    cookie_jar: CookieJar,
) -> impl IntoResponse {

    match google_oauth_init_flow_handler(data, cookie_jar).await {
        Ok(response) => return response.into_response(),
        Err(e) => match e {
            AuthError::OAuthError(ee) => return (StatusCode::OK, Html(format!("OAuth Error: {:?}", ee))).into_response(),
            AuthError::InternalServerError(ee) => return (StatusCode::OK, Html(format!("Error: {:?}", ee))).into_response(),
            _ => return (StatusCode::OK, Html("<h1>Error: Unexpected error occurred</h1>")).into_response()
        }
    }
    
}

pub async fn get_google_oauth_callback(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    Query(callback_params): Query<GoogleOAuthCallbackParams>,
) -> impl IntoResponse {

    match google_oauth_callback_handler(data, cookie_jar, callback_params).await {
        Ok(response) => return response.into_response(),
        Err(e) => match e {
            AuthError::OAuthError(ee) => return (StatusCode::OK, Html(format!("OAuth Error: {:?}", ee))).into_response(),
            AuthError::InternalServerError(ee) => return (StatusCode::OK, Html(format!("Error: {:?}", ee))).into_response(),
            AuthError::AccountNotFound => return (StatusCode::OK, Html("<h1>You do not yet have an account. Create an account on our sign-up page using your Google account's email address and in the future you will be able to sign in with Google.</h1>".to_string())).into_response(),
            _ => return (StatusCode::OK, Html("<h1>Error: Unexpected error occurred</h1>")).into_response()
        }
    }
    
}

// #######################################################################################################################################################
// logout endpoint
// #######################################################################################################################################################

/// Logout the user and return them to the home page
pub async fn get_logout_page(
    cookie_jar: CookieJar,
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>
) -> impl IntoResponse {

    // To get to this page requires auth so we can expect an authorized user variant of auth status instaed of autherror
    let authorized_user = match auth_status {
        AuthStatus::Authorized(user) => user,
        AuthStatus::Unauthorized(_) => panic!("If this happens, check your auth middleware application.")
    };

    match logout_handler(cookie_jar, authorized_user, data).await {
        Ok(response) => return response.into_response(),
        Err(e) => match e {
            AuthError::NotLoggedIn => return Redirect::to("/").into_response(),
            AuthError::InternalServerError(ee) => return (StatusCode::OK, Html(format!("Error: {:?}", ee))).into_response(),
            _ => return (StatusCode::OK, Html("<h1>Error: Unexpected error occurred</h1>")).into_response()
        }
    }
}

