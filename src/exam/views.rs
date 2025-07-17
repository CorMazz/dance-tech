use std::collections::HashMap;
use std::sync::Arc;

use super::errors::ExamError;
use super::handlers::live_grade_handler;
use super::handlers::load_graded_test_from_db;
use super::handlers::post_test_form_handler;
use super::handlers::queue::add_user_to_test_queue;
use super::handlers::queue::remove_user_from_test_queue;
use super::handlers::queue::retrieve_test_queue;
use super::models::PrefilledTestData;
use super::models::QueueEntry;
use super::models::TestGrade;
use crate::AppState;
use crate::app::utils::ErrorTemplate;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
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
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

/// The same template is used to display graded and ungraded tests, using different structs
#[derive(Template)]
#[template(path = "./exam_templates/exam.html", blocks = ["content"])]
pub struct AdministerExamTemplate {
    test: Test,
    /// Used for on the fly test grading and the `submit button`
    test_index: usize,
    is_demo_mode: bool,
    /// If `true`, adds a checkbox that will trigger emailing test results the testee.
    email_functionality_active: bool,
    rts: Routes,
}

#[instrument(skip(data, headers))]
pub async fn get_test_page(
    State(data): State<Arc<AppState>>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Query(prefilled_user_info): Query<PrefilledTestData>,
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
                email_functionality_active: data.smtp_config.is_some(),
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
        AuthStatus::Authorized(proctor) => {
            proctor.user.id
        }
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
            |graded_test| {
                let template = GradedExamTemplate {
                    test: graded_test.test,
                    grade: graded_test.grade,
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
{% call macros::render_grade(grade) %}
"#)]
pub struct TestGradeTemplate {
    grade: TestGrade
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
        AuthStatus::Authorized(proctor) => {
            proctor.user.id
        }
        AuthStatus::Unauthorized(err) => return err.into_response(&headers),
    };
    match live_grade_handler(data, test_index, raw_form, proctor_id) {
        Ok(graded_test) => {
            let template = TestGradeTemplate { grade: graded_test.grade };
            return (StatusCode::OK, Html(render(template))).into_response()
        }
        Err(e) => return e.into_response(&headers)
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
    let test_names = data
        .exam_config
        .test_names
        .clone();

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
}

pub async fn get_user_dashboard_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {

    let template = UserDashboardTemplate {
        rts: ROUTES,
    };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}

/// The queue is only ever intended to be rendered within another page, so it has no content block.
#[derive(Template)]
#[template(path = "./exam_templates/queue.html")]
pub struct QueueTemplate {
    rts: Routes,
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
    let user = match auth_status {
        AuthStatus::Authorized(authorized_user) => Some(authorized_user.user),
        AuthStatus::Unauthorized(_) => None,
    };

    let is_superuser = user.as_ref().is_some_and(crate::auth::models::User::is_superuser);

    match retrieve_test_queue(&data.db, data.exam_config.test_names.clone()).await {
        Ok(queue) => {
            let template = QueueTemplate {
                rts: ROUTES,
                is_superuser,
                is_demo_mode: data.app_config.is_demo_mode,
                queue,
            };
            return (StatusCode::OK, Html(render(template))).into_response()
        }
        Err(e) => return e.into_response(&headers)
    }
}

/// Used for live grading, just render the `Grade` struct
#[derive(Template)]
#[template(path = "./exam_templates/join_queue.html")]
pub struct JoinQueueTemplate {
    rts: Routes,
    test_names: Vec<String>,
    is_signed_in: bool,
}

pub async fn get_join_queue_widget(
    State(data): State<Arc<AppState>>,
    Extension(auth_status): Extension<AuthStatus>,
) -> impl IntoResponse {
    let user = match auth_status {
        AuthStatus::Authorized(authorized_user) => Some(authorized_user.user),
        AuthStatus::Unauthorized(_) => None,
    };

    let is_signed_in = user.is_some();

    let template = JoinQueueTemplate {
        rts: ROUTES,
        test_names: data.exam_config.test_names.clone(),
        is_signed_in
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
    let Some(authorized_user) = (match auth_status {
        AuthStatus::Authorized(user) => Some(user.user),
        AuthStatus::Unauthorized(_) => None,
    }) else {
        return Redirect::to(ROUTES.login).into_response();
    };

    let user_id = match form.user_id.to_lowercase().as_str() {
        "self" => authorized_user.id,
        other => match Uuid::parse_str(other) {
            Ok(uuid) => uuid,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    "Invalid user ID".into_response(),
                )
                    .into_response();
            }
        },
    };

    let queue_result = add_user_to_test_queue(
        &data.db,
        user_id,
        form.test_index as i32,
        data.exam_config.test_names.len(),
        data.exam_config.queue_length,
    )
    .await;

    match queue_result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(ExamError::QueueFull) => (
            StatusCode::CONFLICT,
            "Queue is full".into_response(),
        )
            .into_response(),
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
    Query(form): Query<QueueQueryParameters>,
) -> impl IntoResponse {
    let Some(authorized_user) = (match auth_status {
        AuthStatus::Authorized(user) => Some(user.user),
        AuthStatus::Unauthorized(_) => None,
    }) else {
        return Redirect::to(ROUTES.login).into_response();
    };

    let user_id = match form.user_id.to_lowercase().as_str() {
        "self" => authorized_user.id,
        other => match Uuid::parse_str(other) {
            Ok(uuid) => uuid,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    "Invalid user ID".into_response(),
                )
                    .into_response();
            }
        },
    };

    let queue_result = remove_user_from_test_queue(
        &data.db,
        user_id,
        form.test_index as i32,
    )
    .await;

    match queue_result {
        Ok(..) => StatusCode::OK.into_response(),
        Err(..) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to remove user from queue.".into_response(),
        )
            .into_response(),
    }
}
