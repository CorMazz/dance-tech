use std::collections::BTreeMap;
use uuid::Uuid;

use crate::exam::models::{RadioName, RadioValue, ScoringCategory};
use super::models::{CsvTestTable, RadioButton, RadioOption, TestRow, HtmlTestTable};

/// Take a `DataFrame` and convert it into a nested `TestTable` structure
///
/// A `DataFrame` represents the test questions in wide format, as a human would visualize them and
/// as they will be rendered by the HTML. The HTML itself needs to be generated from the
/// `DataFrame`, and these `TestTable` objects help with that.
pub fn convert_df_to_test_table(df: &CsvTestTable, table_idx: usize) -> HtmlTestTable {
    const DL: &str = "--~--";

    let headers = &df.rows[0];
    let data_rows = &df.rows[1..];

    // Build category → label list, and map column index to (category, label)
    let mut category_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for header in &headers[1..] {
        if let Some((category, label)) = header.split_once(DL) {
            let category = category.trim().to_string();
            let label = label.trim().to_string();
            category_map.entry(category.clone()).or_default().push(label.clone());
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
                let col_idx = headers.iter().position(|h| h == &full_header)
                    .unwrap_or_else(|| panic!("Column {full_header} not found in headers"));

                let point_val = row.get(col_idx)
                    .unwrap_or_else(|| panic!("Missing cell at row {row_idx}, column {col_idx}"));


                let id = Uuid::new_v4();

                options.push(RadioOption {
                    id: id.to_string(),
                    value: RadioValue {
                        label_index: label_idx,
                        point_value: point_val.to_string(),
                    },
                    point_value: point_val.to_string(),
                    checked: label_idx == labels.len() - 1,
                });
            }

            buttons.push(RadioButton {
                name: RadioName {
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

        let left_label = row.first()
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
