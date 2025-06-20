use std::collections::HashMap;
use std::sync::Arc;

use super::handlers::load_graded_test_from_db;
use super::handlers::post_test_form_handler;
use super::models::PrefilledTestData;
use super::models::TestGrade;
use crate::AppState;
use crate::app::utils::ErrorTemplate;
use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::auth::middleware::AuthStatus;
use crate::{
    app::router::{ROUTES, Routes},
    exam::models::{Test, FailureExplanation},
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
    let proctor_id: Uuid;
    match auth_status {
        AuthStatus::Authorized(proctor) => {
            proctor_id = proctor.user.id;
        }
        AuthStatus::Unauthorized(err) => return err.into_response(&headers),
    }

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
    /// Used for on the fly test grading and the `submit button`
    // test_index: usize,
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
        .tests
        .iter()
        .map(|test| test.metadata.test_name.clone())
        .collect();
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
