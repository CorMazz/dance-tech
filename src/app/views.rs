use crate::AppState;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::DisplayRoles;
use crate::auth::models::User;
use crate::auth::utils::search_for_users;
use crate::check_in::handlers::get_products_from_actor;
use crate::check_in::models::Product;
use crate::exam::config::TestDisplayFlag;
use crate::exam::models::Test;
use askama::Template;
use axum::Extension;
use axum::Form;
use axum::extract::Path;
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
use super::handlers::known_grantable_roles;
use super::handlers::post_user_roles_bulk_handler;
use super::handlers::post_user_roles_widget_handler;
use crate::app::router::ROUTES;
use crate::app::router::Routes;
use crate::app::utils::ErrorTemplate;
use crate::exam::errors::ExamError;

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
    hero_source: String,
}

/// Serve the home page template.
///
/// If the request is an HTMX request, it will return just the content block.
pub async fn get_home_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let hero_images = data.hero_gallery.urls().await;
    let hero_json = serde_json::to_string(&hero_images).unwrap_or_else(|_| "[]".to_string());
    let template: HomeTemplate = HomeTemplate {
        rts: ROUTES,
        hero_images,
        hero_json,
        hero_source: data.hero_gallery.source_url().to_string(),
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
        tests: data.exam_config.runtime_tests(),
        products,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}

#[derive(Template)]
#[template(path = "./app_templates/admin_test_card.html")]
struct AdminTestCardTemplate {
    rts: Routes,
    test: Test,
    test_index: usize,
}

#[derive(serde::Deserialize)]
pub struct ToggleTestDisplayForm {
    flag: TestDisplayFlag,
}

/// Flip live grading or show points on a test for this process only.
pub async fn post_toggle_test_display(
    State(data): State<Arc<AppState>>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
    Form(form): Form<ToggleTestDisplayForm>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    let Some(test) = data.exam_config.toggle_display(test_index, form.flag) else {
        return ExamError::TestIndexError.into_response(&headers);
    };

    let template = AdminTestCardTemplate {
        rts: ROUTES,
        test,
        test_index,
    };
    (StatusCode::OK, Html(render(template))).into_response()
}

#[derive(Template)]
#[template(path = "./app_templates/user_roles_widget.html", blocks = ["content", "table_body"])]
pub struct UserRolesTemplate {
    rts: Routes,
    ids: Ids,
    users: Vec<User>,
    known_roles: Vec<String>,
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
        known_roles: known_grantable_roles(&data.exam_config.tests),
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

/// Pasted emails plus the role to grant to every matching account.
#[derive(serde::Deserialize, Debug)]
pub struct BulkGrantForm {
    pub emails: String,
    pub role: String,
}

#[derive(Template)]
#[template(path = "./app_templates/bulk_grant_result.html")]
struct BulkGrantResultTemplate {
    summary: String,
    not_found: Vec<String>,
    invalid: Vec<String>,
}

/// Grant one role to every matching email in a pasted list.
pub async fn post_user_roles_bulk(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
    Form(form): Form<BulkGrantForm>,
) -> impl IntoResponse {
    if matches!(auth_status, AuthStatus::Unauthorized(..))
        || matches!(auth_status, AuthStatus::Authorized(user) if !user.is_admin())
    {
        return Redirect::to(ROUTES.login).into_response();
    }

    match post_user_roles_bulk_handler(form, &data.db).await {
        Ok(outcome) => {
            let template = BulkGrantResultTemplate {
                summary: outcome.summary,
                not_found: outcome.not_found,
                invalid: outcome.invalid,
            };
            (StatusCode::OK, Html(render(template))).into_response()
        }
        Err(e) => e.into_response(&headers),
    }
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
                known_roles: known_grantable_roles(&data.exam_config.tests),
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
