use std::collections::BTreeMap;

use polars::frame::DataFrame;
use crate::exam::models::ScoringCategory;

use super::models::{RadioButton, RadioOption, TestRow, TestTable};

/// Take a `DataFrame` and convert it into a nested `TestTable` structure
///
/// A `DataFrame` represents the test questions in wide format, as a human would visualize them and
/// as they will be rendered by the HTML. The HTML itself needs to be generated from the
/// `DataFrame`, and these `TestTable` objects help with that.
pub fn convert_df_to_test_table(df: &DataFrame, table_idx: &usize) -> TestTable {
    let index_col = df.column("index").expect("Missing 'index' column");
    let index = index_col.str().expect("'index' column must be parseable to string");
    
    const DL: &str = "-.-"; // Delimiter

    // Group columns by category (e.g., footwork, timing)
    let mut category_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for col_name in df.get_column_names().iter().skip(1) {
        if let Some((category, label)) = col_name.split_once(DL) {
            category_map
                .entry(category.to_string())
                .or_default()
                .push(label.to_string());
        } else {
            panic!("Column {col_name} is missing `{DL}` delimiter");
        }
    }

    let mut scoring_categories = Vec::new();
    let mut rows = Vec::new();

    for row_idx in 0..df.height() {
        let mut buttons = Vec::new();

        for (category, labels) in &category_map {
            let mut options = Vec::new();

            for (label_idx, label) in labels.iter().enumerate() {
                let full_col = format!("{category}{DL}{label}");
                let col = df.column(&full_col).unwrap();
                let val = col.get(row_idx).unwrap().to_string();

                let id = format!(
                    "{category}{DL}row-{row_idx}{DL}label-{label_idx}"
                );

                options.push(RadioOption {
                    id,
                    value: val,
                    checked: label_idx == labels.len() - 1, // Last option checked by default
                });
            }

            buttons.push(RadioButton {
                name: format!("{table_idx}{DL}{category}{DL}row-{row_idx}"),
                options,
            });

            // Only push these once (they're global across the table)
            if row_idx == 0 {
                scoring_categories.push( ScoringCategory {
                    name: category.to_string(),
                    values: labels.clone(),
                });
            }
        }

        rows.push(TestRow { 
            buttons,
            left_label: index.get(row_idx).unwrap().to_string(),
            left_label_subtext: String::new(),
            right_label: String::new(),
        });
    }

    TestTable {
        scoring_categories,
        rows,
    }
}
