use serde_json::{from_value, to_value};
use sqlx::{Pool, Postgres};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tracing::{debug, error, instrument};
use uuid::Uuid;

use crate::AppState;
use crate::exam::models::{CsvTestTable, HtmlTestTable, RadioButton, RadioOption, TestRow};
use crate::exam::models::{RadioName, RadioValue, ScoringCategory};

use super::errors::ExamError;
use super::models::GradedTest;

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
pub fn parse_test_form(
    form: HashMap<String, String>,
) -> Result<(Vec<(RadioName, RadioValue)>, Vec<usize>), ExamError> {
    let mut competencies: Vec<(RadioName, RadioValue)> = Vec::new();
    let mut bonus_indices: Vec<usize> = Vec::new();

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
        } else {
            error!("Unknown form key: {}", key);
            return Err(ExamError::ParseError);
        }
    }
    Ok((competencies, bonus_indices))
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
    let json_value = to_value(&test).expect("GradedTest should serialize to JSON");

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
            ExamError::InternalServerError(Some("Unable to save the test to the database. Try again later or contact the site administrator.".to_string()))
    })?;

    debug!(%test_id, "Saved graded test to database.");
    Ok(())
}

/// Loads a graded test from the database.
///
/// The test was originally stored as JSONB, so it deserializes the JSON into a GradedTest object.
#[instrument(skip(db))]
pub async fn load_graded_test_from_db(
    test_id: Uuid,
    db: &Pool<Postgres>,
) -> Result<GradedTest, ExamError> {
    let row = sqlx::query!(
        r#"
        SELECT test_data
        FROM graded_exams
        WHERE id = $1
        "#,
        test_id
    )
    .fetch_optional(db)
    .await
    .map_err(|err| {
        error!(%err, "Database error while fetching graded test.");
        ExamError::InternalServerError(Some(
            "Unable to fetch the test. Please try again.".to_string(),
        ))
    })?;

    let row = row.ok_or_else(|| {
        error!("No graded test found with ID: `{}`", test_id);
        ExamError::GradedTestNotFound
    })?;

    let graded_test: GradedTest = from_value(row.test_data).map_err(|err| {
        error!(%err, "Deserialization error while loading GradedTest.");
        ExamError::InternalServerError(Some(
            "Test data could not be processed. Please contact the site administrator.".to_string(),
        ))
    })?;
    debug!(%graded_test.id, "Loaded graded test from the database.");
    Ok(graded_test)
}

/// Used to return a `Grade` object to the live grading endpoint. 
#[instrument(skip(form, data, test_index))]
pub fn live_grade_handler(
    data: Arc<AppState>,
    test_index: usize,
    form: HashMap<String, String>,
    proctor_id: Uuid,
) -> Result<GradedTest, ExamError> {
    debug!("Received raw test form {:#?}", form);
    let (competencies, bonus_indices) = parse_test_form(form)?;
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

    let graded_test = test.grade(competencies, bonus_indices, proctor_id)?;
    debug!("Successfully graded test.");
    Ok(graded_test)
}

/// Receive the form from a test page, parse it, grade the test, and save it to the database.
#[instrument(skip(form, data, test_index))]
pub async fn post_test_form_handler(
    data: Arc<AppState>,
    test_index: usize,
    form: HashMap<String, String>,
    proctor_id: Uuid,
) -> Result<(), ExamError> {
    let graded_test = live_grade_handler(data.clone(), test_index, form, proctor_id)?;
    save_graded_test_to_db(graded_test, &data.db).await?;
    Ok(())
}
