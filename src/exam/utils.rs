//! For functions that are used within the handlers.

use super::errors::ExamError;
use super::models::{GradedExamFilter, GradedTest, QueueEntry};
use crate::auth::models::Roles;
use crate::auth::models::User;
use crate::exam::models::{
    CsvTestTable, ExamStatus, HtmlTestTable, RadioButton, RadioOption, TestRow,
};
use crate::exam::models::{RadioName, RadioValue, ScoringCategory};
use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use serde_json::{from_value, to_value};
use sqlx::PgPool;
use sqlx::Row;
use sqlx::types::Json;
use sqlx::{Execute, Pool, Postgres, QueryBuilder};
use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use tracing::{debug, error, instrument};
use tracing::{trace, warn};
use uuid::Uuid;

/// Take a `DataFrame` and convert it into a nested `TestTable` structure
///
/// A `DataFrame` represents the test questions in wide format, as a human would visualize them and
/// as they will be rendered by the HTML. The HTML itself needs to be generated from the
/// `DataFrame`, and these `TestTable` objects help with that.
pub fn convert_df_to_test_table(
    df: &CsvTestTable,
    container_idx: usize,
    table_idx: usize,
) -> HtmlTestTable {
    const DL: &str = "--~--";

    let headers = &df.rows[0];
    let data_rows = &df.rows[1..];

    // Build category → label list, and map column index to (category, label)
    let mut category_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for header in &headers[1..] {
        if let Some((category, label)) = header.split_once(DL) {
            let category = category.trim().to_string();
            let label = label.trim().to_string();
            category_map
                .entry(category.clone())
                .or_default()
                .push(label.clone());
        } else {
            panic!("Add the damn delimiter.")
        }
    }

    let mut scoring_categories = Vec::new();
    let mut rows = Vec::new();

    for (row_idx, row) in data_rows.iter().enumerate() {
        let mut buttons = Vec::new();

        for (category_idx, (category, labels)) in category_map.iter().enumerate() {
            let mut options = Vec::new();

            for (label_idx, label) in labels.iter().enumerate() {
                // Reconstruct full column name to find its index
                let full_header = format!("{category}{DL}{label}");
                let col_idx = headers
                    .iter()
                    .position(|h| h == &full_header)
                    .unwrap_or_else(|| panic!("Column {full_header} not found in headers"));

                let full_point_val = row
                    .get(col_idx)
                    .unwrap_or_else(|| panic!("Missing cell at row {row_idx}, column {col_idx}"));

                let point_val: usize;
                let is_failing: bool;
                if let Some(point_val_str) = full_point_val.strip_suffix("f") {
                    point_val = point_val_str
                        .parse::<usize>()
                        .expect("Unable to parse point_value to `usize");
                    is_failing = true;
                } else {
                    point_val = full_point_val
                        .parse::<usize>()
                        .expect("Unable to parse point_value to `usize");
                    is_failing = false;
                }

                let id = Uuid::new_v4();

                options.push(RadioOption {
                    id: id.to_string(),
                    value: RadioValue {
                        label_index: label_idx,
                        point_value: point_val,
                        fails_test: is_failing,
                    },
                    checked: label_idx == labels.len() - 1,
                });
            }

            buttons.push(RadioButton {
                name: RadioName {
                    container_index: container_idx,
                    table_index: table_idx,
                    category_index: category_idx,
                    row_index: row_idx,
                },
                options,
            });

            if row_idx == 0 {
                scoring_categories.push(ScoringCategory {
                    name: category.clone(),
                    values: labels.clone(),
                });
            }
        }

        let left_label = row
            .first()
            .unwrap_or_else(|| panic!("Missing 'index' column at row {row_idx}"))
            .to_string();

        rows.push(TestRow {
            buttons,
            left_label,
            left_label_subtext: String::new(),
            right_label: String::new(),
        });
    }

    HtmlTestTable {
        scoring_categories,
        rows,
    }
}

/// Given a raw submitted test form, parse it into the metadata, competencies, and bonus indices.
#[instrument(skip(form))]
#[allow(clippy::type_complexity)]
pub fn parse_test_form(
    form: HashMap<String, String>,
) -> Result<(Vec<(RadioName, RadioValue)>, Vec<usize>, String), ExamError> {
    let mut competencies: Vec<(RadioName, RadioValue)> = Vec::new();
    let mut bonus_indices: Vec<usize> = Vec::new();
    let mut email = String::new();

    for (key, value) in form {
        if let Some(json_str) = key.strip_prefix("competency") {
            let name: RadioName = serde_json::from_str(json_str).map_err(|err| {
                error!(%err, "Unable to parse RadioName from `{json_str}`.");
                ExamError::ParseError
            })?;
            let val: RadioValue = serde_json::from_str(&value).map_err(|err| {
                error!(%err, "Unable to parse RadioValue from `{json_str}`.");
                ExamError::ParseError
            })?;
            competencies.push((name, val));
        } else if let Some(index_str) = key.strip_prefix("bonus_index") {
            let bonus_index: usize = index_str.parse().map_err(|err| {
                error!(%err, "Unable to parse usize from `{index_str}`.");
                ExamError::ParseError
            })?;

            bonus_indices.push(bonus_index);
        } else if key == "email" {
            email = value;
        } else {
            error!("Unknown form key: {}", key);
            return Err(ExamError::ParseError);
        }
    }
    Ok((competencies, bonus_indices, email))
}

/// I already dealt with the headache of trying to store a graded test as something fancy in the
/// database and split it into all of its different components. Let's take the easy route this time
/// and just drop the whole shebang in there as JSONB.
#[instrument(skip(test, db))]
pub async fn save_graded_test_to_db(
    test: GradedTest,
    db: &Pool<Postgres>,
) -> Result<(), ExamError> {
    let test_id = test.id;
    let json_value = to_value(&test).map_err(|err| {
        error!(%err, "Unable to serialize the graded test to JSONB.");
        ExamError::FatalInternalServerError
    })?;

    sqlx::query!(
        r#"
        INSERT INTO graded_exams (id, test_data)
        VALUES ($1, $2)
        "#,
        test_id,
        json_value
    )
    .execute(db)
    .await
    .map_err(|err| {
        error!(%err, %test_id, "Unable to save test to the database.");
        ExamError::DatabaseError
    })?;

    debug!(%test_id, "Saved graded test to database.");
    Ok(())
}

/// Loads a graded test from the database.
///
/// The test was originally stored as JSONB, so it deserializes the JSON into a `GradedTest` object.
/// Returns a tuple of (`GradedTest`, `created_at` timestamp).
/// `created_at` was added after the application was released to production, so we couldn't just include it
/// as part of the `GradedTest` struct.
#[instrument(skip(db))]
pub async fn load_graded_test_from_db(
    test_id: Uuid,
    db: &Pool<Postgres>,
) -> Result<(GradedTest, Option<DateTime<Utc>>), ExamError> {
    let row = sqlx::query!(
        r#"
        SELECT test_data, created_at
        FROM graded_exams
        WHERE id = $1
        "#,
        test_id
    )
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(%err, "Database error while fetching graded test.");
        ExamError::FatalInternalServerError
    })?;

    let row = row.ok_or_else(|| {
        error!("No graded test found with ID: `{}`", test_id);
        ExamError::GradedTestNotFound
    })?;

    let graded_test: GradedTest = from_value(row.test_data).map_err(|err| {
        error!(%err, "Deserialization error while loading GradedTest.");
        ExamError::FatalInternalServerError
    })?;
    debug!(%graded_test.id, "Loaded graded test from the database.");
    Ok((graded_test, row.created_at))
}

/// Read a string into a CSV. Used to load the tests from files.
pub fn parse_csv(csv_data: &str) -> CsvTestTable {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_data.as_bytes());

    let headers = rdr
        .headers()
        .expect("Unable to parse headers")
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let mut all_rows = vec![headers];

    for result in rdr.records() {
        let record = result.unwrap();
        let row = record
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        all_rows.push(row);
    }

    CsvTestTable { rows: all_rows }
}

#[instrument(skip(db))]
pub async fn add_user_to_test_queue(
    db: &PgPool,
    user_id: Uuid,
    test_index: i32,
    n_tests: usize,
    max_length: usize,
) -> Result<(), ExamError> {
    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM exam_queue")
        .fetch_one(db)
        .await
        .map_err(|e| {
            error!("Error adding user to queue: {e}");
            ExamError::DatabaseError
        })?
        .unwrap_or(0);

    #[allow(clippy::cast_possible_wrap)]
    if count >= max_length as i64 {
        return Err(ExamError::QueueFull);
    }

    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    if test_index > (n_tests as i32 - 1) {
        error!("Invalid test index.");
        return Err(ExamError::DatabaseError);
    }

    let result = sqlx::query!(
        "INSERT INTO exam_queue (user_id, test_index)
         VALUES ($1, $2)
         ON CONFLICT (user_id, test_index) DO NOTHING",
        user_id,
        test_index
    )
    .execute(db)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Error adding user to queue: {e}");
            Err(ExamError::DatabaseError)
        }
    }
}

/// Remove a specific user from the test queue based on their `user_id` and `test_index`.
/// Returns a boolean indicating if a user was removed or not.
#[instrument(skip(db))]
pub async fn remove_user_from_test_queue(
    db: &PgPool,
    user_id: Uuid,
    test_index: i32,
) -> Result<bool, ExamError> {
    let res = sqlx::query!(
        "DELETE FROM exam_queue WHERE user_id = $1 AND test_index = $2",
        user_id,
        test_index
    )
    .execute(db)
    .await
    .map_err(|e| {
        error!("Error removing a user from the queue: {e}");
        ExamError::DatabaseError
    })?;

    let was_user_removed = res.rows_affected() > 0;

    if !was_user_removed {
        warn!("A user and id combination was not found in the database and thus wasn't removed.");
    }

    Ok(was_user_removed)
}

/// Get the full list of users currently in the queue.
#[instrument(skip(db))]
pub async fn retrieve_test_queue(
    db: &PgPool,
    test_names: Vec<String>,
) -> Result<Vec<QueueEntry>, ExamError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            u.id,
            u.first_name,
            u.last_name,
            u.email,
            u.password,
            u.roles as "roles: Json<HashSet<Roles>>",
            u.created_at,
            u.updated_at,
            q.test_index
        FROM exam_queue q
        JOIN users u ON q.user_id = u.id
        ORDER BY q.inserted_at
        "#
    )
    .fetch_all(db)
    .await
    .map_err(|e| {
        error!("Error retrieving queue: {e}");
        ExamError::DatabaseError
    })?;

    let entries = rows
        .into_iter()
        .map(|r| {
            #[allow(clippy::cast_sign_loss)]
            let test_name = test_names.get(r.test_index as usize).ok_or_else(|| {
                error!("Invalid test index {} in queue row. This shouldn't happen. Try restarting the app which will clear the queue.", r.test_index);
                ExamError::DatabaseError
            })?
            .to_string();

            Ok(QueueEntry {
                user: User {
                    id: r.id,
                    first_name: r.first_name,
                    last_name: r.last_name,
                    email: r.email,
                    password: r.password,
                    roles: r.roles,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                },
                test_index: r.test_index,
                test_name,
            })
        })
        .collect::<Result<Vec<_>, ExamError>>()?;
    Ok(entries)
}

/// A struct to contain the items necessary to explain a `GradedTest`.
/// The test is linked to the user by ID, which isn't useful for displaying,
/// so we want to grab the users by ID so that we can display their info on
/// the info page.
pub struct FilteredExamResult {
    pub test: GradedTest,
    pub testee: User,
    pub proctor: User,
    pub taken_at: DateTime<Utc>,
}

/// Retrieve all tests that fit the parameters included in the filter
/// Returns the (Test, Testee, Proctor)
#[instrument(skip(db))]
pub async fn query_filtered_exams(
    filter: GradedExamFilter,
    db: &Pool<Postgres>,
) -> Result<Vec<FilteredExamResult>, ExamError> {
    let mut builder = QueryBuilder::new(
        r"SELECT 
          graded_exams.test_data, 
          graded_exams.created_at,
          row_to_json(testee_user.*) AS testee_user, 
          row_to_json(proctor_user.*) AS proctor_user
        FROM graded_exams 
        JOIN users AS testee_user ON testee_user.id = (graded_exams.test_data->>'testee_id')::uuid 
        JOIN users AS proctor_user ON proctor_user.id = (graded_exams.test_data->>'proctor_id')::uuid",
    );

    if let Some(pass_or_fail) = filter.pass_or_fail {
        match pass_or_fail {
            ExamStatus::Passing => {
                builder
                    .push(" AND test_data->'grade'->>'is_passing' = ")
                    .push_bind("true");
            }
            ExamStatus::Failing => {
                builder
                    .push(" AND test_data->'grade'->>'is_passing' = ")
                    .push_bind("false");
            }
            ExamStatus::Both => {
                // No filter needed; include both passing and failing exams
            }
        }
    }

    if let Some(testee_ids) = &filter.testee_ids
        && !testee_ids.is_empty()
    {
        builder
            .push(" AND test_data->>'testee_id' IN (")
            .push_values(
                // the iterator that yields each bind value
                testee_ids.iter(),
                // how to bind each element
                |mut b, id| {
                    b.push_bind(id.to_string());
                },
            )
            .push(")");
    }

    if let Some(proctor_ids) = &filter.proctor_ids
        && !proctor_ids.is_empty()
    {
        builder
            .push(" AND test_data->>'proctor_id' IN (")
            .push_values(proctor_ids.iter(), |mut b, id| {
                b.push_bind(id.to_string());
            })
            .push(")");
    }

    if let Some(test_name) = &filter.test_name {
        debug!("Test Name: {test_name}");
        builder
            .push(" AND (test_data->'test'->'metadata'->>'test_name') % ")
            .push_bind(test_name);
    }

    builder.push(" LIMIT ");
    #[allow(clippy::cast_possible_wrap)]
    builder.push_bind(filter.per_page as i64);
    let offset = (filter.page.saturating_sub(1)) * filter.per_page;
    builder.push(" OFFSET ");
    #[allow(clippy::cast_possible_wrap)]
    builder.push_bind(offset as i64);

    let query = builder.build();
    debug!(sql = %query.sql());

    let rows = query.fetch_all(db).await.map_err(|err| {
        error!(%err, "Error retrieving test data from database.");
        ExamError::DatabaseError
    })?;

    let mut results = Vec::new();
    for row in rows {
        let json_value: serde_json::Value = row.try_get("test_data").map_err(|err| {
            error!(%err, "Error retrieving test data from row.");
            ExamError::DatabaseError
        })?;
        let graded_test: GradedTest = serde_json::from_value(json_value).map_err(|err| {
            error!(%err, "Error deserializing graded test.");
            ExamError::FatalInternalServerError
        })?;

        trace!("Row: {row:#?}");

        let json_value: serde_json::Value = row.try_get("testee_user").map_err(|err| {
            error!(%err, "Error retrieving testee data from row.");
            ExamError::DatabaseError
        })?;

        let testee: User = serde_json::from_value(json_value).map_err(|err| {
            error!(%err, "Error deserializing testee.");
            ExamError::FatalInternalServerError
        })?;

        let json_value: serde_json::Value = row.try_get("proctor_user").map_err(|err| {
            error!(%err, "Error retrieving proctor data from row.");
            ExamError::DatabaseError
        })?;

        let proctor: User = serde_json::from_value(json_value).map_err(|err| {
            error!(%err, "Error deserializing proctor.");
            ExamError::FatalInternalServerError
        })?;
        let created_at_option: Option<DateTime<Utc>> =
            row.try_get("created_at").map_err(|err| {
                error!(%err, "Error retrieving created_at from row.");
                ExamError::DatabaseError
            })?;
        let created_at = created_at_option.unwrap_or_else(Utc::now);
        results.push(FilteredExamResult {
            test: graded_test,
            testee,
            proctor,
            taken_at: created_at,
        });
    }

    Ok(results)
}
