use crate::AppState;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::DisplayRoles;
use crate::auth::models::User;
use crate::auth::utils::search_for_users;
use crate::check_in::handlers::get_products_from_actor;
use crate::check_in::models::Product;
use crate::exam::models::Test;
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
use std::sync::Arc;
use uuid::Uuid;

use super::handlers::delete_user_roles_widget_handler;
use super::handlers::post_user_roles_widget_handler;
use crate::app::router::ROUTES;
use crate::app::router::Routes;
use crate::app::utils::ErrorTemplate;

/// The one absolute truth for html element IDs that are used across multiple templates
pub struct Ids {
    /// Used when rendering the `user_roles_widget`. If the request comes with an HX-Trigger
    /// header with this name, will render only the table and update just that.
    pub user_role_form: &'static str,
    /// Used to target the body of the user table on the modify roles widget
    pub user_table_body: &'static str,
}

pub const IDS: Ids = Ids {
    user_role_form: "search-users-form",
    user_table_body: "user-table-body",
};

#[derive(Template)]
#[template(path = "./app_templates/home.html", blocks = ["content"])]
pub struct HomeTemplate {
    rts: Routes,
    hero_images: Vec<String>,
    hero_json: String,
}

/// Serve the home page template.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_home_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let hero_images = data.hero_gallery.urls(&data.http_client).await;
    let hero_json = serde_json::to_string(&hero_images).unwrap_or_else(|_| "[]".to_string());
    let template: HomeTemplate = HomeTemplate {
        rts: ROUTES,
        hero_images,
        hero_json,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

#[derive(Template)]
#[template(path = "./app_templates/admin_dashboard.html", blocks = ["content"])]
pub struct AdminDashboardTemplate {
    rts: Routes,
    tests: Vec<Test>,
    products: Vec<Product>,
}

/// The admin dashboard lets admins view details about all available products, and
/// lets administrators add/remove roles from users
pub async fn get_admin_dashboard(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    let products = match get_products_from_actor(&data).await {
        Ok(products) => products,
        Err(err) => return err.into_response(&headers),
    };

    let template = AdminDashboardTemplate {
        rts: ROUTES,
        tests: data.exam_config.tests.clone(),
        products,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

#[derive(Template)]
#[template(path = "./app_templates/user_roles_widget.html", blocks = ["content", "table_body"])]
pub struct UserRolesTemplate {
    rts: Routes,
    ids: Ids,
    users: Vec<User>,
}

#[derive(serde::Deserialize)]
pub struct UserQuery {
    #[serde(default)]
    query: Option<String>,
}

/// This widget minimally allows admins to see user roles and add/remove roles from users.
///
/// More advanced manipulation can be done directly on the database using `PGAdmin`.
pub async fn get_user_roles_widget(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(user_query): Query<UserQuery>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    let users = match user_query.query {
        None => Vec::new(),
        Some(user_query) => match search_for_users(user_query, &data.db).await {
            Ok(users) => users,
            Err(err) => return err.into_response(&headers),
        },
    };

    let template = UserRolesTemplate {
        rts: ROUTES,
        ids: IDS,
        users,
    };

    if matches!(headers.get("HX-Trigger"), Some(div_id) if div_id == IDS.user_role_form) {
        (StatusCode::OK, Html(render(template.as_table_body()))).into_response()
    } else if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

/// A struct used to either add or remove roles from a user
#[derive(serde::Deserialize, Debug)]
pub struct ModifyUserQuery {
    pub user_id: Uuid,
    pub role: String,
}

/// This widget minimally allows admins to see user roles and add/remove roles from users.
///
/// The delete method allows admins to remove roles from certain users.
pub async fn delete_user_roles_widget(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<ModifyUserQuery>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    match delete_user_roles_widget_handler(query, &data.db).await {
        Ok(..) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(&headers),
    }
}

/// This widget minimally allows admins to see user roles and add/remove roles from users.
///
/// The post method allows admins to add roles to users
pub async fn post_user_roles_widget(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
    Form(query): Form<ModifyUserQuery>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    match post_user_roles_widget_handler(query, &data.db).await {
        Ok(users) => {
            let template = UserRolesTemplate {
                rts: ROUTES,
                ids: IDS,
                users,
            };
            (StatusCode::OK, Html(render(template.as_table_body()))).into_response()
        }
        Err(e) => e.into_response(&headers),
    }
}

#[derive(Template)]
#[template(path = "./app_templates/contact.html", blocks = ["content"])]
pub struct ContactTemplate {
    rts: Routes,
}

pub async fn get_contact_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template = ContactTemplate { rts: ROUTES };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

#[derive(Template)]
#[template(path = "./app_templates/terms.html", blocks = ["content"])]
pub struct TermsTemplate {
    rts: Routes,
}

pub async fn get_terms_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template = TermsTemplate { rts: ROUTES };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

#[derive(Template)]
#[template(path = "./app_templates/privacy.html", blocks = ["content"])]
pub struct PrivacyTemplate {
    rts: Routes,
}

pub async fn get_privacy_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template = PrivacyTemplate { rts: ROUTES };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

/// Serve the error 404 not found page
pub async fn error_404_page() -> impl IntoResponse {
    let template: ErrorTemplate = ErrorTemplate {
        rts: ROUTES,
        error_message: "404 Requested Path Not Found".to_string(),
    };
    (StatusCode::NOT_FOUND, Html(render(template)))
}
