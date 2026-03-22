use super::errors::ExamError;
use super::handlers::live_grade_handler;
use super::handlers::post_test_form_handler;
use super::handlers::search_exam_widget_handler;
use super::models::ExamStatus;
use super::models::QueueEntry;
use super::models::TestGrade;
use super::utils::{
    FilteredExamResult, add_user_to_test_queue, load_graded_test_from_db,
    remove_user_from_test_queue, retrieve_test_queue,
};
use crate::AppState;
use crate::app::utils::ErrorTemplate;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::User;
use crate::auth::utils::{get_user_by_email, get_user_by_id, search_for_users};
use crate::{
    app::router::{ROUTES, Routes},
    exam::models::{FailureExplanation, Test},
};
use askama::Template;
use axum::Extension;
use axum::Form;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Redirect;
use axum_extra::extract::Host;
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// The one absolute truth for html element IDs that are used across multiple templates
pub struct Ids {
    /// Used when rendering the `user_roles_widget`. If the request comes with an HX-Trigger
    /// header with this name, will render only the table and update just that.
    pub test_form: &'static str,

    /// Used on the user dashboard and the queue widget to force the queue to update
    /// when a user clicks the join queue button.
    pub queue_container: &'static str,

    /// Same as above
    pub refresh_queue_event: &'static str,

    /// Same as above. This is the div that launches the hx-get request to populate the
    /// `queue_container`
    pub queue_reload_handler: &'static str,
}

pub const IDS: Ids = Ids {
    test_form: "search-tests-form",
    queue_container: "queue-container",
    refresh_queue_event: "refresh-queue",
    queue_reload_handler: "queue-reload-handler",
};

/// The same template is used to display graded and ungraded tests, using different structs
#[derive(Template)]
#[template(path = "./exam_templates/exam.html", blocks = ["content"])]
pub struct AdministerExamTemplate {
    test: Test,
    /// Used for on the fly test grading and the `submit button`
    test_index: usize,
    is_demo_mode: bool,
    // /// If `true`, adds a checkbox that will trigger emailing test results the testee.
    // email_functionality_active: bool,
    /// Prefills the user on the test page
    testee_email: String,
    rts: Routes,
}

/// Used to assign a test to a specific user when proctoring
#[derive(Deserialize, Debug)]
pub struct UserEmailForm {
    /// If not specified, just use a blank string
    #[serde(default)]
    pub email: String,
}

#[instrument(skip(data, headers))]
pub async fn get_test_page(
    State(data): State<Arc<AppState>>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Query(testee_email): Query<UserEmailForm>,
) -> impl IntoResponse {
    data.exam_config.tests.get(test_index).map_or_else(
        || {
            let template = ErrorTemplate {
                error_message: "There is no test with that ID number.".to_string(),
                rts: ROUTES,
            };

            if is_htmx_request(&headers) {
                (StatusCode::OK, Html(render(template.as_content())))
            } else {
                (StatusCode::OK, Html(render(template)))
            }
        },
        |test| {
            let template = AdministerExamTemplate {
                test: test.clone(),
                test_index,
                is_demo_mode: data.app_config.is_demo_mode,
                // email_functionality_active: data.smtp_config.is_some(),
                testee_email: testee_email.email,
                rts: ROUTES,
            };

            if is_htmx_request(&headers) {
                (StatusCode::OK, Html(render(template.as_content())))
            } else {
                (StatusCode::OK, Html(render(template)))
            }
        },
    )
}

/// Handles parsing the test form, saving the graded test to the database, and emailing test results to the testee.
#[instrument(skip(data, auth_status, headers, raw_form))]
pub async fn post_test_form(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Host(server_root_url): Host,
    Form(raw_form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let proctor_id = match auth_status {
        AuthStatus::Authorized(proctor) => proctor.id,
        AuthStatus::Unauthorized(err) => return err.into_response(&headers),
    };

    if let Err(e) = post_test_form_handler(data, test_index, raw_form, proctor_id).await {
        return e.into_response(&headers);
    }
    Redirect::to(ROUTES.proctor_dashboard).into_response()
    // let proctor = match auth_status {
    //     AuthStatus::Authorized(user) => Proctor { id: user.user.id, first_name: user.user.first_name, last_name: user.user.last_name},
    //     AuthStatus::Unauthorized(e) => return e.into_response(&headers)
    // };
    //
    // // By virtue of this existing, they want the email sent.
    // let testee_wants_email_sent = test.get("send_email_results").is_some();
    //
    // if let Some(test_definition) = data.exam_config.tests.get(test_index) {
    //     match parse_test_form_data(test, test_definition.clone(), Some(proctor)) {
    //         Ok(graded_test) => {
    //             match save_test_to_database(&data.db, graded_test).await {
    //                 Ok(testee_id) => {
    //                     if let (
    //                         Some(smtp_config),
    //                         Some(smtp_mailer),
    //                         true) = (
    //                             data.smtp_config.clone(),
    //                             data.smtp_mailer.clone(),
    //                             testee_wants_email_sent
    //                         ) {
    //                         // tokio::spawn(async move {
    //                         //     if let Err(e) = send_email(&data.db, &smtp_mailer, smtp_config, testee_id, server_root_url).await {
    //                         //         eprintln!("Failed to send email: {:?}", e);
    //                         //     }
    //                         // });
    //                     };
    //                     Redirect::to("/dashboard").into_response()
    //                 },
    //                 Err(e) => error_response(&format!("Error saving test to database: {:?}", e)).into_response()
    //             }
    //         },
    //         Err(e) => error_response(&format!("Error parsing test form data: {:?}", e)).into_response()
    //     }
    // } else {
    //     error_response(&format!("Invalid test index ({}) in URL", test_index)).into_response()
    // }
}

/// The same template is used to display graded and ungraded tests, using different structs
#[derive(Template)]
#[template(path = "./exam_templates/exam.html", blocks = ["content"])]
pub struct GradedExamTemplate {
    test: Test,
    grade: TestGrade,
    taken_at: Option<DateTime<Utc>>,
    rts: Routes,
}

#[instrument(skip(data, headers))]
pub async fn get_graded_test_page(
    State(data): State<Arc<AppState>>,
    Path(test_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    load_graded_test_from_db(test_id, &data.db)
        .await
        .map_or_else(
            |err| err.into_response(&headers),
            |(graded_test, taken_at)| {
                let template = GradedExamTemplate {
                    test: graded_test.test,
                    grade: graded_test.grade,
                    taken_at,
                    rts: ROUTES,
                };

                if is_htmx_request(&headers) {
                    (StatusCode::OK, Html(render(template.as_content()))).into_response()
                } else {
                    (StatusCode::OK, Html(render(template))).into_response()
                }
            },
        )
}

/// Used for live grading, just render the `Grade` struct
#[derive(Template)]
#[template(
    ext = "txt",
    source = r#"
{% import "./exam_templates/macros.html" as macros %}
{% call macros::render_grade(grade, date_taken) %}
"#
)]
pub struct TestGradeTemplate {
    grade: TestGrade,
    date_taken: Option<DateTime<Utc>>,
}

/// Let users get feedback on the fly by grading partially completed tests and returning just the
/// grade object. This will always be an HTMX request.
#[instrument(skip(data, auth_status, headers, raw_form))]
pub async fn post_live_grading(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Host(server_root_url): Host,
    Form(raw_form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let proctor_id = match auth_status {
        AuthStatus::Authorized(proctor) => proctor.id,
        AuthStatus::Unauthorized(err) => return err.into_response(&headers),
    };
    match live_grade_handler(data, test_index, raw_form, proctor_id).await {
        Ok(graded_test) => {
            let template = TestGradeTemplate {
                grade: graded_test.grade,
                date_taken: None, // On live grading, just don't show the current date
            };
            return (StatusCode::OK, Html(render(template))).into_response();
        }
        Err(e) => return e.into_response(&headers),
    }
}

#[derive(Template)]
#[template(path = "./exam_templates/proctor_dashboard.html", blocks = ["content"])]
pub struct ProctorDashboardTemplate {
    rts: Routes,
    test_names: Vec<String>,
}

pub async fn get_proctor_dashboard_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let test_names = data.exam_config.test_names.clone();

    let template: ProctorDashboardTemplate = ProctorDashboardTemplate {
        rts: ROUTES,
        test_names,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

#[derive(Template)]
#[template(path = "./exam_templates/user_dashboard.html", blocks = ["content"])]
pub struct UserDashboardTemplate {
    rts: Routes,
    ids: Ids,
}

pub async fn get_user_dashboard_page(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let template = UserDashboardTemplate {
        rts: ROUTES,
        ids: IDS,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

/// The queue is only ever intended to be rendered within another page, so it has no content block.
#[derive(Template)]
#[template(path = "./exam_templates/queue_widget.html")]
pub struct QueueTemplate {
    rts: Routes,
    ids: Ids,
    /// If the current user has the role `Admin` or `Proctor`.
    is_superuser: bool,
    is_demo_mode: bool,
    queue: Vec<QueueEntry>,
}

/// This can fail if there are users in the queue signed up to take a test that no longer exists.
/// That invariant is maintained by clearing the queue everytime the app starts, since that ensures
/// that only valid `test_index` (indices into the vec of test names) are in the db.
pub async fn get_queue_widget(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    let user = auth_status.ok();
    let is_superuser = user
        .as_ref()
        .is_some_and(crate::auth::models::User::is_superuser);

    match retrieve_test_queue(&data.db, data.exam_config.test_names.clone()).await {
        Ok(queue) => {
            let template = QueueTemplate {
                rts: ROUTES,
                ids: IDS,
                is_superuser,
                is_demo_mode: data.app_config.is_demo_mode,
                queue,
            };
            (StatusCode::OK, render(template)).into_response()
        }
        Err(e) => e.into_response(&headers),
    }
}

/// Used for live grading, just render the `Grade` struct
#[derive(Template)]
#[template(path = "./exam_templates/join_queue_widget.html")]
pub struct JoinQueueTemplate {
    rts: Routes,
    ids: Ids,
    test_names: Vec<String>,
    is_signed_in: bool,
}

pub async fn get_join_queue_widget(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    let user = auth_status.ok();
    let is_signed_in = user.is_some();

    let template = JoinQueueTemplate {
        rts: ROUTES,
        ids: IDS,
        test_names: data.exam_config.test_names.clone(),
        is_signed_in,
    };

    (StatusCode::OK, Html(render(template)))
}

#[derive(Deserialize)]
pub struct QueueQueryParameters {
    pub user_id: String,
    pub test_index: usize,
}

/// Add a user to the queue.
///
/// Eventually add toasts to give users feedback on whether joining the queue
/// was successful
pub async fn post_queue_widget(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    Query(form): Query<QueueQueryParameters>,
) -> impl IntoResponse {
    let user = match auth_status.require_auth() {
        Ok(user) => user,
        Err(e) => return e.into_response(),
    };

    let user_id = match form.user_id.to_lowercase().as_str() {
        "self" => user.id,
        other => match Uuid::parse_str(other) {
            Ok(uuid) => uuid,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "Invalid user ID".into_response())
                    .into_response();
            }
        },
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let test_index = form.test_index as i32;

    let queue_result = add_user_to_test_queue(
        &data.db,
        user_id,
        test_index,
        data.exam_config.test_names.len(),
        data.exam_config.queue_length,
    )
    .await;

    match queue_result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(ExamError::QueueFull) => {
            (StatusCode::CONFLICT, "Queue is full".into_response()).into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to join queue".into_response(),
        )
            .into_response(),
    }
}

/// Removes a user from the queue upon receiving a delete request. If called with a request header HX-Trigger equal to
/// "administer-test-button", will redirect to the proper administer test page with the query parameters
/// equal to the queue user's information. If there is no response header, just deletes the user and returns empty html.
pub async fn delete_queue_widget(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    headers: axum::http::HeaderMap,
    Query(form): Query<QueueQueryParameters>,
) -> impl IntoResponse {
    let user = match auth_status.require_auth() {
        Ok(user) => user,
        Err(e) => return e.into_response(),
    };

    let user_id = match form.user_id.to_lowercase().as_str() {
        "self" => user.id,
        other => match Uuid::parse_str(other) {
            Ok(uuid) => uuid,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "Invalid user ID".into_response())
                    .into_response();
            }
        },
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let test_index = form.test_index as i32;
    let queue_result = remove_user_from_test_queue(&data.db, user_id, test_index).await;

    let is_administer_test_request = headers
        .get("HX-Trigger")
        .is_some_and(|val| val == "administer-test-button");

    match queue_result {
        Ok(..) => {
            if is_administer_test_request {
                if let Ok(Some(user)) = get_user_by_id(&user_id, &data.db).await {
                    return Redirect::to(
                        &ROUTES.administer_exam_for_user(&form.test_index, &user.email),
                    )
                    .into_response();
                }
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to get user by id.",
                )
                    .into_response();
            }
            StatusCode::OK.into_response()
        }
        Err(..) => (
            // The error is logged within the remove_user_from_test_queue function
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to remove user from queue.".into_response(),
        )
            .into_response(),
    }
}

#[derive(Template)]
#[template(path = "./exam_templates/user_info_widget.html")]
pub struct UserInfoWidgetTemplate {
    pub user: Option<User>,
}

/// Used to display information about the user that we are proctoring a test for.
pub async fn get_user_info_widget(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    Query(form): Query<UserEmailForm>,
) -> impl IntoResponse {
    // I tried returning a result type from the function, but it was a PITA
    if let Err(e) = auth_status.require_auth() {
        return e.into_response();
    }

    let user = get_user_by_email(&form.email, &data.db).await;

    match user {
        Ok(user) => {
            let template = UserInfoWidgetTemplate { user };
            (StatusCode::OK, render(template)).into_response()
        }
        Err(..) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("Failed to get user info. Please try again later.".to_string()),
        )
            .into_response(),
    }
}

/// Will be placed inside of a `<datalist>` element
#[derive(Template)]
#[template(
    ext = "txt",
    source = r#"
{% for user in users %}
    <option value={{ user.email }}></option>
{% endfor %}
"#
)]
pub struct UserEmailAutoCompleteTemplate {
    pub users: Vec<User>,
}

/// Used for autocomplete on the test page when entering user emails
pub async fn get_user_autocomplete(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
    Query(form): Query<UserEmailForm>,
) -> impl IntoResponse {
    if let Err(e) = auth_status.require_auth() {
        return e.into_response();
    }

    let users = search_for_users(form.email, &data.db).await;

    match users {
        Ok(users) => {
            let template = UserEmailAutoCompleteTemplate { users };
            (StatusCode::OK, render(template)).into_response()
        }
        Err(..) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get user info. Please try again later.".into_response(),
        )
            .into_response(),
    }
}

#[derive(Template)]
#[template(path = "./exam_templates/search_tests_widget.html", blocks = ["content", "table"])]
pub struct SearchTestsWidgetTemplate {
    rts: Routes,
    ids: Ids,
    filter: SearchTestFilters,
    filtered_exam_info: Vec<FilteredExamResult>,
    has_next_page: bool,
    /// Used to hide the `testee` search box from non-superusers
    is_superuser: bool,
    /// If not signed in, tells users to sign in to see their results
    is_signed_in: bool,
}

/// Query parameters for the search test widget
#[derive(Debug, Deserialize, Default)]
pub struct SearchTestFilters {
    /// The search string to match on testee first name, last name, or email
    pub testee_query: Option<String>,
    /// The search string to match on proctor first name, last name, or email
    pub proctor_query: Option<String>,
    pub test_name: Option<String>,
    pub pass_or_fail: Option<ExamStatus>,

    #[serde(default = "default_page")]
    pub page: usize,

    #[serde(default = "default_per_page")]
    pub per_page: usize,
}
const fn default_page() -> usize {
    1
}
const fn default_per_page() -> usize {
    10
}

/// Search for tests and display information about them (name, date taken, etc.)
/// Admin users can search by testee name and see any test results. Non-admin
/// users can only see their own test results.
pub async fn get_search_tests_widget(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
    Query(filter): Query<SearchTestFilters>,
) -> impl IntoResponse {
    let Some(user) = auth_status.ok() else {
        let template = SearchTestsWidgetTemplate {
            rts: ROUTES,
            ids: IDS,
            filtered_exam_info: Vec::new(),
            filter,
            has_next_page: false,
            is_superuser: false,
            is_signed_in: false,
        };
        if is_htmx_request(&headers) {
            return (StatusCode::OK, Html(render(template.as_content()))).into_response();
        }
        return (StatusCode::OK, Html(render(template))).into_response();
    };

    let (graded_tests, has_next_page) =
        match search_exam_widget_handler(&filter, &user, &data.db).await {
            Ok(res) => res,
            Err(e) => return e.into_response(&headers),
        };

    let template = SearchTestsWidgetTemplate {
        rts: ROUTES,
        ids: IDS,
        filtered_exam_info: graded_tests,
        filter,
        has_next_page,
        is_superuser: user.is_superuser(),
        is_signed_in: true,
    };

    if matches!(headers.get("HX-Trigger"), Some(div_id) if div_id == IDS.test_form) {
        (StatusCode::OK, Html(render(template.as_table()))).into_response()
    } else if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content()))).into_response()
    } else {
        (StatusCode::OK, Html(render(template))).into_response()
    }
}
