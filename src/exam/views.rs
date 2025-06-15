use std::collections::HashMap;
use crate::exam::models::Proctor;
use axum::response::Redirect;
use std::sync::Arc;

use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::app::utils::ErrorTemplate;
use crate::auth::middleware::AuthStatus;
use crate::exam::models::FullTestSummary;
use crate::exam::models::PrefilledTestData;
use crate::AppState;
use askama::Template;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::Html;
use axum::response::IntoResponse;
use axum_extra::extract::Host;
use axum::Extension;
use reqwest::StatusCode;
use tracing::instrument;
use crate::app::filters;
use crate::{app::router::{Routes, ROUTES}, exam::models::Test};

#[derive(Template)]
#[template(path = "./exam_templates/exam.html", blocks = ["content"])] 
pub struct ExamTemplate {
    test: Test,
    prefilled_user_info: PrefilledTestData,
    test_summary: Option<FullTestSummary>,
    test_index: usize, // Used for on the fly test grading
    is_demo_mode: bool,
    email_functionality_active: bool,
    rts: Routes
}

#[instrument(skip(data, headers))]
pub async fn get_test_page(
    State(data): State<Arc<AppState>>,
    Path(test_index): Path<usize>,
    headers: axum::http::HeaderMap,
    Query(prefilled_user_info): Query<PrefilledTestData>,
) -> impl IntoResponse  {

    data.exam_config.tests.get(test_index).map_or_else(|| {
        let template = ErrorTemplate { error_message: "There is no test with that ID number.".to_string(), rts: ROUTES };

        if is_htmx_request(&headers) {
            (StatusCode::OK, Html(render(template.as_content())))
        } else {
            (StatusCode::OK, Html(render(template)))
        }
    }, |test| {
        let template = ExamTemplate {
            test: test.clone(),
            prefilled_user_info,
            test_summary: None,
            test_index,
            is_demo_mode: data.app_config.is_demo_mode,
            email_functionality_active: data.smtp_config.is_some(),
            rts: ROUTES
        };
        
        if is_htmx_request(&headers) {
            (StatusCode::OK, Html(render(template.as_content())))
        } else {
            (StatusCode::OK, Html(render(template)))
        }
        
    })
}

// /// Handles parsing the test form, saving the graded test to the database, and emailing test results to the testee.
// pub async fn post_test_form(
//     State(data): State<Arc<AppState>>,
//     Extension(auth_status): Extension<AuthStatus>,
//     Path(test_index): Path<i32>,
//     Host(server_root_url): Host,
//     Form(test): Form<HashMap<String, String>>,
// ) -> impl IntoResponse {
//
//     let proctor = match auth_status {
//         AuthStatus::Authorized(user) => Proctor { id: user.user.id, first_name: user.user.first_name, last_name: user.user.last_name},
//         AuthStatus::Unauthorized(e) => return error_response(&format!("Unauthorized: {:?}", e)).into_response()
//     };
//
//     // By virtue of this existing, they want the email sent.
//     let testee_wants_email_sent = test.get("send_email_results").is_some();
//
//     if let Some(test_definition) = data.test_configurations.tests.get(test_index as usize) {
//         match parse_test_form_data(test, test_definition.clone(), Some(proctor)) {
//             Ok(graded_test) => {
//                 match save_test_to_database(&data.db, graded_test).await {
//                     Ok(testee_id) => {
//                         if let (
//                             Some(smtp_config), 
//                             Some(smtp_mailer), 
//                             true) = (
//                                 data.smtp_config.clone(), 
//                                 data.smtp_mailer.clone(),
//                                 testee_wants_email_sent
//                             ) {
//                             tokio::spawn(async move {
//                                 if let Err(e) = send_email(&data.db, &smtp_mailer, smtp_config, testee_id, server_root_url).await {
//                                     eprintln!("Failed to send email: {:?}", e);
//                                 }
//                             });
//                         };
//                         Redirect::to("/dashboard").into_response()
//                     },
//                     Err(e) => error_response(&format!("Error saving test to database: {:?}", e)).into_response()
//                 }
//             },
//             Err(e) => error_response(&format!("Error parsing test form data: {:?}", e)).into_response()
//         }
//     } else {
//         error_response(&format!("Invalid test index ({}) in URL", test_index)).into_response()
//     }
// }


#[derive(Template)]
#[template(path = "./exam_templates/proctor_dashboard.html", blocks = ["content"])] 
pub struct ProctorDashboardTemplate {
    rts: Routes,
    test_names: Vec<String>
}

pub async fn get_proctor_dashboard_page(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse  {
    let test_names = data.exam_config.tests.iter().map(|test| test.metadata.test_name.clone()).collect();
    let template: ProctorDashboardTemplate = ProctorDashboardTemplate { rts: ROUTES, test_names };

    if is_htmx_request(&headers) {
        (StatusCode::OK, Html(render(template.as_content())))
    } else {
        (StatusCode::OK, Html(render(template)))
    }
}


