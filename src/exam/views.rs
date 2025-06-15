use std::sync::Arc;

use crate::app::utils::is_htmx_request;
use crate::app::utils::render;
use crate::app::utils::ErrorTemplate;
use crate::exam::models::FullTestSummary;
use crate::exam::models::PrefilledTestData;
use crate::AppState;
use askama::Template;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::response::Html;
use axum::response::IntoResponse;
use reqwest::StatusCode;
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

