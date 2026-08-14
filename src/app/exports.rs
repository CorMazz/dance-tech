//! Admin CSV downloads of accounts and graded exam sittings.

use crate::AppState;
use crate::app::router::ROUTES;
use crate::auth::errors::AuthError;
use crate::auth::middleware::AuthStatus;
use crate::auth::models::{DisplayRoles, Roles, User};
use crate::auth::utils::list_all_users;
use crate::exam::errors::ExamError;
use crate::exam::utils::{FilteredExamResult, query_all_graded_exams};
use axum::Extension;
use axum::body::Body;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, instrument};
use uuid::Uuid;

const EASTERN: chrono_tz::Tz = chrono_tz::America::New_York;

/// Download every account as `users.csv`. Admin only. Never includes passwords.
#[instrument(skip(data, headers))]
pub async fn get_export_users(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> Response {
    if !is_admin(&auth_status) {
        return Redirect::to(ROUTES.login).into_response();
    }

    let users = match list_all_users(&data.db).await {
        Ok(users) => users,
        Err(err) => return err.into_response(&headers),
    };

    match write_users_csv(&users) {
        Ok(bytes) => csv_attachment("dancetech-users", bytes),
        Err(err) => {
            error!(%err, "Error writing users CSV.");
            AuthError::FatalInternalServerError.into_response(&headers)
        }
    }
}

/// Download every graded sitting as `exam_attempts.csv`. Admin only.
#[instrument(skip(data, headers))]
pub async fn get_export_exams(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> Response {
    if !is_admin(&auth_status) {
        return Redirect::to(ROUTES.login).into_response();
    }

    let exams = match query_all_graded_exams(&data.db).await {
        Ok(exams) => exams,
        Err(err) => return err.into_response(&headers),
    };

    match write_exams_csv(&exams) {
        Ok(bytes) => csv_attachment("dancetech-exam-attempts", bytes),
        Err(err) => {
            error!(%err, "Error writing exam attempts CSV.");
            ExamError::FatalInternalServerError.into_response(&headers)
        }
    }
}

/// One row per current role on each account, joined to the earliest passing exam that grants it.
#[instrument(skip(data, headers))]
pub async fn get_export_user_roles(
    State(data): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Extension(auth_status): Extension<AuthStatus>,
) -> Response {
    if !is_admin(&auth_status) {
        return Redirect::to(ROUTES.login).into_response();
    }

    let users = match list_all_users(&data.db).await {
        Ok(users) => users,
        Err(err) => return err.into_response(&headers),
    };
    let exams = match query_all_graded_exams(&data.db).await {
        Ok(exams) => exams,
        Err(err) => return err.into_response(&headers),
    };

    match write_user_roles_csv(&users, &exams) {
        Ok(bytes) => csv_attachment("dancetech-user-roles", bytes),
        Err(err) => {
            error!(%err, "Error writing user roles CSV.");
            ExamError::FatalInternalServerError.into_response(&headers)
        }
    }
}

fn is_admin(auth_status: &AuthStatus) -> bool {
    matches!(auth_status, AuthStatus::Authorized(user) if user.is_admin())
}

fn csv_attachment(stem: &str, bytes: Vec<u8>) -> Response {
    let filename = format!("{stem}-{}.csv", eastern_date_stamp(Utc::now()));
    let disposition = format!("attachment; filename=\"{filename}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn format_eastern(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&EASTERN)
        .format("%Y-%m-%d %H:%M %Z")
        .to_string()
}

fn format_eastern_opt(dt: Option<DateTime<Utc>>) -> String {
    dt.map(format_eastern).unwrap_or_default()
}

fn eastern_date_stamp(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&EASTERN).format("%Y-%m-%d").to_string()
}

#[derive(Serialize)]
struct UserCsvRow<'a> {
    user_id: Uuid,
    first_name: &'a str,
    last_name: &'a str,
    email: &'a str,
    roles: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct ExamCsvRow<'a> {
    user_id: Uuid,
    first_name: &'a str,
    last_name: &'a str,
    email: &'a str,
    test_name: &'a str,
    taken_at: String,
    passed: bool,
    roles_conferred: String,
    score: usize,
    percent: f32,
    max_score: usize,
    passing_percent: f32,
    proctor_name: String,
    proctor_email: &'a str,
    exam_id: Uuid,
}

fn write_users_csv(users: &[User]) -> Result<Vec<u8>, csv::Error> {
    let mut buf = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut buf);
        for user in users {
            writer.serialize(UserCsvRow {
                user_id: user.id,
                first_name: &user.first_name,
                last_name: &user.last_name,
                email: &user.email,
                roles: user.roles.0.display_roles(),
                created_at: format_eastern_opt(user.created_at),
                updated_at: format_eastern_opt(user.updated_at),
            })?;
        }
        writer.flush()?;
    }
    Ok(buf)
}

fn roles_conferred(sitting: &FilteredExamResult) -> String {
    if sitting.test.grade.is_passing {
        sitting
            .test
            .test
            .metadata
            .config
            .get_granted_roles()
            .display_roles()
    } else {
        String::new()
    }
}

fn write_exams_csv(exams: &[FilteredExamResult]) -> Result<Vec<u8>, csv::Error> {
    let mut buf = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut buf);
        for sitting in exams {
            writer.serialize(ExamCsvRow {
                user_id: sitting.testee.id,
                first_name: &sitting.testee.first_name,
                last_name: &sitting.testee.last_name,
                email: &sitting.testee.email,
                test_name: &sitting.test.test.metadata.test_name,
                taken_at: format_eastern(sitting.taken_at),
                passed: sitting.test.grade.is_passing,
                roles_conferred: roles_conferred(sitting),
                score: sitting.test.grade.achieved_score,
                percent: sitting.test.grade.achieved_percent,
                max_score: sitting.test.grade.max_score,
                passing_percent: sitting.test.grade.minimum_percent,
                proctor_name: format!(
                    "{} {}",
                    sitting.proctor.first_name, sitting.proctor.last_name
                ),
                proctor_email: &sitting.proctor.email,
                exam_id: sitting.test.id,
            })?;
        }
        writer.flush()?;
    }
    Ok(buf)
}

#[derive(Serialize)]
struct UserRoleCsvRow<'a> {
    user_id: Uuid,
    first_name: &'a str,
    last_name: &'a str,
    email: &'a str,
    role: String,
    taken_at: String,
    test_name: String,
    exam_id: String,
}

fn earliest_passing_by_user_role(
    exams: &[FilteredExamResult],
) -> HashMap<(Uuid, Roles), &FilteredExamResult> {
    let mut best = HashMap::new();
    for sitting in exams {
        if !sitting.test.grade.is_passing {
            continue;
        }
        for role in sitting.test.test.metadata.config.get_granted_roles() {
            best.entry((sitting.testee.id, role))
                .and_modify(|current: &mut &FilteredExamResult| {
                    if sitting.taken_at < current.taken_at {
                        *current = sitting;
                    }
                })
                .or_insert(sitting);
        }
    }
    best
}

fn write_user_roles_csv(
    users: &[User],
    exams: &[FilteredExamResult],
) -> Result<Vec<u8>, csv::Error> {
    let earned = earliest_passing_by_user_role(exams);
    let mut buf = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut buf);
        for user in users {
            let mut roles: Vec<&Roles> = user.roles.0.iter().collect();
            roles.sort_by_key(|role| role.to_string());
            for role in roles {
                let sitting = earned.get(&(user.id, role.clone())).copied();
                writer.serialize(UserRoleCsvRow {
                    user_id: user.id,
                    first_name: &user.first_name,
                    last_name: &user.last_name,
                    email: &user.email,
                    role: role.to_string(),
                    taken_at: sitting
                        .map(|s| format_eastern(s.taken_at))
                        .unwrap_or_default(),
                    test_name: sitting
                        .map(|s| s.test.test.metadata.test_name.clone())
                        .unwrap_or_default(),
                    exam_id: sitting.map(|s| s.test.id.to_string()).unwrap_or_default(),
                })?;
            }
        }
        writer.flush()?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::Roles;
    use crate::exam::models::{GradedTest, Metadata, Test, TestConfig, TestGrade};
    use chrono::TimeZone;
    use sqlx::types::Json;
    use std::collections::HashSet;

    fn sample_user(password: &str) -> User {
        let created = Utc.with_ymd_and_hms(2026, 1, 15, 17, 30, 0).unwrap();
        User {
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            first_name: "Abby".into(),
            last_name: "Smith".into(),
            email: "abby@example.com".into(),
            password: password.into(),
            roles: Json(HashSet::from([
                Roles::Admin,
                Roles::Dynamic("advanced-leader".into()),
            ])),
            created_at: Some(created),
            updated_at: Some(created),
        }
    }

    #[test]
    fn users_csv_omits_password_and_uses_eastern_times() {
        let csv =
            String::from_utf8(write_users_csv(&[sample_user("SUPER-SECRET")]).unwrap()).unwrap();
        assert!(csv.contains("user_id,first_name,last_name,email,roles,created_at,updated_at"));
        assert!(csv.contains("abby@example.com"));
        assert!(csv.contains("admin"));
        assert!(csv.contains("advanced-leader"));
        assert!(csv.contains("2026-01-15 12:30 EST"));
        assert!(!csv.contains("SUPER-SECRET"));
        assert!(!csv.contains("password"));
    }

    #[test]
    fn exams_csv_one_row_per_sitting() {
        let testee = sample_user("secret-testee");
        let mut proctor = sample_user("secret-proctor");
        proctor.id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        proctor.first_name = "Pat".into();
        proctor.last_name = "Proctor".into();
        proctor.email = "pat@example.com".into();

        let sitting = FilteredExamResult {
            test: GradedTest {
                id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                test: Test {
                    metadata: Metadata {
                        test_name: "Leader Test".into(),
                        minimum_percent: 80.0,
                        max_score: 100,
                        config: TestConfig {
                            live_grading: false,
                            show_point_values: false,
                            grants_roles: vec!["advanced-leader".into()],
                        },
                    },
                    containers: vec![],
                    bonus_items: vec![],
                },
                grade: TestGrade {
                    achieved_score: 90,
                    achieved_percent: 90.0,
                    minimum_percent: 80.0,
                    max_score: 100,
                    is_passing: true,
                    failure_explanations: vec![],
                },
                proctor_id: proctor.id,
                testee_id: testee.id,
            },
            testee,
            proctor,
            taken_at: Utc.with_ymd_and_hms(2026, 8, 14, 22, 0, 0).unwrap(),
        };

        let csv = String::from_utf8(write_exams_csv(&[sitting]).unwrap()).unwrap();
        assert!(csv.contains("test_name,taken_at,passed,roles_conferred,score,percent"));
        assert!(csv.contains("Leader Test"));
        assert!(csv.contains("advanced-leader"));
        assert!(csv.contains("Pat Proctor"));
        assert!(csv.contains("pat@example.com"));
        assert!(csv.contains("2026-08-14 18:00 EDT"));
        assert!(!csv.contains("secret-testee"));
        assert!(!csv.contains("secret-proctor"));
    }

    #[test]
    fn exams_csv_roles_conferred_blank_when_failed() {
        let testee = sample_user("secret-testee");
        let mut proctor = sample_user("secret-proctor");
        proctor.first_name = "Pat".into();
        proctor.last_name = "Proctor".into();
        proctor.email = "pat@example.com".into();

        let sitting = FilteredExamResult {
            test: GradedTest {
                id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                test: Test {
                    metadata: Metadata {
                        test_name: "Leader Test".into(),
                        minimum_percent: 80.0,
                        max_score: 100,
                        config: TestConfig {
                            live_grading: false,
                            show_point_values: false,
                            grants_roles: vec!["advanced-leader".into()],
                        },
                    },
                    containers: vec![],
                    bonus_items: vec![],
                },
                grade: TestGrade {
                    achieved_score: 10,
                    achieved_percent: 10.0,
                    minimum_percent: 80.0,
                    max_score: 100,
                    is_passing: false,
                    failure_explanations: vec![],
                },
                proctor_id: proctor.id,
                testee_id: testee.id,
            },
            testee,
            proctor,
            taken_at: Utc.with_ymd_and_hms(2026, 8, 14, 22, 0, 0).unwrap(),
        };

        let csv = String::from_utf8(write_exams_csv(&[sitting]).unwrap()).unwrap();
        let data_row = csv.lines().nth(1).unwrap();
        assert!(!data_row.contains("advanced-leader"));
    }

    fn sample_proctor() -> User {
        let mut proctor = sample_user("secret-proctor");
        proctor.id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        proctor.first_name = "Pat".into();
        proctor.last_name = "Proctor".into();
        proctor.email = "pat@example.com".into();
        proctor
    }

    fn sample_sitting(
        testee: User,
        passed: bool,
        grants_roles: Vec<String>,
        taken_at: DateTime<Utc>,
        exam_id: Uuid,
    ) -> FilteredExamResult {
        let proctor = sample_proctor();
        FilteredExamResult {
            test: GradedTest {
                id: exam_id,
                test: Test {
                    metadata: Metadata {
                        test_name: "Leader Test".into(),
                        minimum_percent: 80.0,
                        max_score: 100,
                        config: TestConfig {
                            live_grading: false,
                            show_point_values: false,
                            grants_roles,
                        },
                    },
                    containers: vec![],
                    bonus_items: vec![],
                },
                grade: TestGrade {
                    achieved_score: if passed { 90 } else { 10 },
                    achieved_percent: if passed { 90.0 } else { 10.0 },
                    minimum_percent: 80.0,
                    max_score: 100,
                    is_passing: passed,
                    failure_explanations: vec![],
                },
                proctor_id: proctor.id,
                testee_id: testee.id,
            },
            testee,
            proctor,
            taken_at,
        }
    }

    #[test]
    fn user_roles_csv_joins_exam_date_or_leaves_blank() {
        let user = sample_user("secret");
        let exam_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let sitting = sample_sitting(
            user.clone(),
            true,
            vec!["advanced-leader".into()],
            Utc.with_ymd_and_hms(2026, 8, 14, 22, 0, 0).unwrap(),
            exam_id,
        );

        let csv = String::from_utf8(write_user_roles_csv(&[user], &[sitting]).unwrap()).unwrap();
        assert!(csv.contains("user_id,first_name,last_name,email,role,taken_at,test_name,exam_id"));
        let admin_row = csv
            .lines()
            .find(|line| line.contains(",admin,"))
            .expect("admin role row");
        assert!(!admin_row.contains("Leader Test"));
        assert!(!admin_row.contains(&exam_id.to_string()));
        let leader_row = csv
            .lines()
            .find(|line| line.contains(",advanced-leader,"))
            .expect("advanced-leader role row");
        assert!(leader_row.contains("2026-08-14 18:00 EDT"));
        assert!(leader_row.contains("Leader Test"));
        assert!(leader_row.contains(&exam_id.to_string()));
        assert!(!csv.contains("secret"));
    }

    #[test]
    fn user_roles_csv_ignores_failing_exams_and_keeps_earliest_pass() {
        let user = sample_user("secret");
        let first_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let later_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap();
        let fail_id = Uuid::parse_str("55555555-5555-5555-5555-555555555555").unwrap();
        let exams = vec![
            sample_sitting(
                user.clone(),
                false,
                vec!["advanced-leader".into()],
                Utc.with_ymd_and_hms(2026, 1, 1, 17, 0, 0).unwrap(),
                fail_id,
            ),
            sample_sitting(
                user.clone(),
                true,
                vec!["advanced-leader".into()],
                Utc.with_ymd_and_hms(2026, 6, 1, 16, 0, 0).unwrap(),
                first_id,
            ),
            sample_sitting(
                user.clone(),
                true,
                vec!["advanced-leader".into()],
                Utc.with_ymd_and_hms(2026, 8, 1, 16, 0, 0).unwrap(),
                later_id,
            ),
        ];

        let csv = String::from_utf8(write_user_roles_csv(&[user], &exams).unwrap()).unwrap();
        let leader_row = csv
            .lines()
            .find(|line| line.contains(",advanced-leader,"))
            .unwrap();
        assert!(leader_row.contains(&first_id.to_string()));
        assert!(!leader_row.contains(&later_id.to_string()));
        assert!(!leader_row.contains(&fail_id.to_string()));
        assert!(leader_row.contains("2026-06-01 12:00 EDT"));
    }
}
