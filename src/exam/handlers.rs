use super::errors::ExamError;
use super::models::{GradedExamFilter, GradedTest};
use super::utils::{FilteredExamResult, save_graded_test_to_db};
use super::views::SearchTestFilters;
use crate::AppState;
use crate::auth::models::User;
use crate::auth::utils::{get_user_by_email, grant_roles, search_for_users};
use crate::exam::utils::{parse_test_form, query_filtered_exams};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, instrument};
use uuid::Uuid;

/// Used to return a `Grade` object to the live grading endpoint.
#[instrument(skip(form, data, test_index))]
pub async fn live_grade_handler(
    data: Arc<AppState>,
    test_index: usize,
    form: HashMap<String, String>,
    proctor_id: Uuid,
) -> Result<GradedTest, ExamError> {
    debug!("Received raw test form {:#?}", form);
    let (competencies, bonus_indices, _email) = parse_test_form(form)?;
    debug!(
        "Parsed form into competencies: {competencies:#?} and bonus_indices: {bonus_indices:#?}."
    );
    let test = data
        .exam_config
        .tests
        .get(test_index)
        .ok_or_else(|| {
            error!("Invalid test index: `{test_index}.");
            ExamError::TestIndexError
        })?
        .clone();

    let graded_test = test.grade(competencies, bonus_indices, proctor_id, Uuid::new_v4())?;
    debug!("Successfully graded test.");
    Ok(graded_test)
}

/// Used to grade a finished test. Fails if the user was not specified.
#[instrument(skip(form, data, test_index))]
pub async fn final_grade_handler(
    data: Arc<AppState>,
    test_index: usize,
    form: HashMap<String, String>,
    proctor_id: Uuid,
) -> Result<(GradedTest, User), ExamError> {
    debug!("Received raw test form {:#?}", form);
    let (competencies, bonus_indices, email) = parse_test_form(form)?;
    debug!(
        "Parsed form into competencies: {competencies:#?} and bonus_indices: {bonus_indices:#?}."
    );
    let test = data
        .exam_config
        .tests
        .get(test_index)
        .ok_or_else(|| {
            error!("Invalid test index: `{test_index}.");
            ExamError::TestIndexError
        })?
        .clone();

    let testee = get_user_by_email(&email, &data.db)
        .await
        .map_err(|_| ExamError::DatabaseError)?
        .ok_or(ExamError::UserNotFound)?;

    let graded_test = test.grade(competencies, bonus_indices, proctor_id, testee.id)?;
    debug!("Successfully graded test.");
    Ok((graded_test, testee))
}

/// Receive the form from a test page, parse it, grade the test, and save it to the database.
#[instrument(skip(form, data, test_index))]
pub async fn post_test_form_handler(
    data: Arc<AppState>,
    test_index: usize,
    form: HashMap<String, String>,
    proctor_id: Uuid,
) -> Result<(), ExamError> {
    let (graded_test, testee) =
        final_grade_handler(data.clone(), test_index, form, proctor_id).await?;

    grant_roles(
        testee,
        graded_test.test.metadata.config.get_granted_roles(),
        &data.db,
    )
    .await
    .map_err(|_| ExamError::DatabaseError)?;

    save_graded_test_to_db(graded_test, &data.db).await?;
    Ok(())
}

/// Searches for exams given a series of filter parameters
/// Querying user is used to only display the user's own test results
/// for non-admin users.
#[instrument(skip(db))]
pub async fn search_exam_widget_handler(
    filter: &SearchTestFilters,
    querying_user: &User,
    db: &Pool<Postgres>,
) -> Result<(Vec<FilteredExamResult>, bool), ExamError> {
    let testee_ids = if querying_user.is_superuser() {
        match &filter.testee_query {
            Some(query) if !query.is_empty() => {
                let users = search_for_users(query.clone(), db)
                    .await
                    .map_err(|_| ExamError::DatabaseError)?;

                Some(users.into_iter().map(|user| user.id).collect::<Vec<_>>())
            }
            _ => None,
        }
    } else {
        Some(vec![querying_user.id])
    };

    let proctor_ids = match &filter.proctor_query {
        Some(query) if !query.is_empty() => Some(
            search_for_users(query.clone(), db)
                .await
                .map_err(|_| ExamError::DatabaseError)?
                .into_iter()
                .map(|user| user.id)
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };

    let test_name = match &filter.test_name {
        Some(s) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    };

    let query_input = GradedExamFilter {
        testee_ids,
        proctor_ids,
        pass_or_fail: filter.pass_or_fail.clone(),
        test_name,
        page: filter.page,
        per_page: filter.per_page + 1,
    };

    debug!("Graded Exam Filter: {:#?}", query_input);

    let mut exams = query_filtered_exams(query_input, db).await?;

    let has_next_page = exams.len() > filter.per_page;
    if has_next_page {
        exams.truncate(filter.per_page);
    }

    Ok((exams, has_next_page))
}
