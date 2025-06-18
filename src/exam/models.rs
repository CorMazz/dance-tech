use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub metadata: Metadata,
    /// Contains multiple different tables within it. Each table has a new set of headers
    pub containers: Vec<TestContainer>,
    pub bonus_items: Option<Vec<BonusItem>>,
}

/// A bonus item adds bonus points on the test.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BonusItem {
    pub test_id: Option<Uuid>,
    pub name: String,
    pub score: i32,
    pub achieved: Option<bool>,
}

/// Metadata contains information about the test needed for the application to work.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub test_name: String,
    pub minimum_percent: f32,
    pub max_score: i32,
    pub config: TestConfig,
}

/// Changes how the test is displayed to the user.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    pub live_grading: bool,
    pub show_point_values: bool,
}

/// A table is a collection of sections (groups) of questions/competencies on a test.
/// There may be multiple sections (different sets of grade categories) on a table
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestContainer {
    /// The name to be displayed at the top of this container. Ie. 'Pattern Scoring'
    pub name: String,
    pub tables: Vec<HtmlTestTable>,
    /// The original `DataFrames` that are loaded in from the configuration files and converted into
    /// the `TestTable` objects
    pub dfs: Vec<CsvTestTable>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CsvTestTable {
    pub rows: Vec<Vec<String>>
}

// impl Into<HtmlTestTable> for CsvTestTable {
//
// }

/// A table can also be thought of as a `DataFrame` style structure with a column multi-index to
/// allow for multiple scoring categories within a single table (if that were possible in Polars).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HtmlTestTable {
    pub scoring_categories: Vec<ScoringCategory>,
    pub rows: Vec<TestRow>
}

/// The text labels for the different scores
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoringCategory {
    /// The name of the concept being scored. IE: 'Footwork'. Can be `""`.
    pub name: String,
    /// The words used to describe what a score means. IE: 'Perfect', 'Right Concept', 'Not So Much'
    pub values: Vec<String>
}

/// A single row in a test table. A row may have two different scoring categories, such as when
/// scoring patterns you can judge the `Footwork` and `Timing` categories. Each one of those
/// categories will have a button, and the button will have various options.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestRow {
    /// The number of buttons should correspond with the number of category labels in for the
    /// corresponding `TestTable`
    pub buttons: Vec<RadioButton>,
    /// The label that goes on the left of the table. Can be thought of as the row index. In the
    /// case of the `Patterns` container, this would contain the individual pattern names.
    pub left_label: String,
    /// Additional information to put in small text under the left label. If empty, is ignored.
    pub left_label_subtext: String,
    /// The label that goes on the right side of the table. In the case of the `Technique`
    /// container, this would contain the antitheses to the technique. Ie. if the left label were
    /// 'Body Lead' then the right label would be 'Arm Lead', indicating poor lead technique. If
    /// empty, is ignored.
    pub right_label: String,
}

/// A group of html `<input type="radio">` that all share the same name (and are thus all linked).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadioButton {
    /// This is what gets sent when the form is submitted as the key (assuming the form is parsed
    /// as a flat `HashMap`).
    pub name: RadioName,
    /// The number of options should correspond with the number of score labels for the corresponding
    /// `TestTable`.
    pub options: Vec<RadioOption>
}

/// The `name` field on a radio button is included in the post request when a form is submitted.
/// We can deserialize the keys in the form request into this `RadioName` type. This allows us to 
/// correspond the form results to the original test they came from so that we can grade the test.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(clippy::struct_field_names)]
pub struct RadioName {
    /// The index of the table on the form that the `RadioButton` belongs to.
    pub table_index: usize,
    /// The index of the scoring category on the form that the `RadioButton` belongs to.
    pub category_index: usize,
    /// The index of the row of the table on the form that the `RadioButton` belongs to.
    pub row_index: usize,
}

/// A single button within a `RadioItem`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadioOption {
    /// Must be unique for each element, is used to assign `<label>` element to the button (which
    /// makes the button prettier than a stupid looking filled circle from 1985).
    pub id: String,
    /// This is the value that gets sent with the key, which is a serialized string containing the
    /// point value and the index of that score within the `CsvTestTable`
    pub value: RadioValue,
    /// The point value of a specified answer on a question. Displayed within the buttons.
    pub point_value: String,
    /// Determine if this is the one that starts checked
    pub checked: bool
}


/// The `value` field on a radio button is included in the post request when a form is submitted.
/// We can deserialize the values in the form request into this `RadioValue` type. This allows us to 
/// correspond the form results to the original test they came from so that we can grade the test.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadioValue {
    /// The index of the column of the table on the form that this `RadioOption` belongs to.
    pub label_index: usize,
    /// The point value of a specified answer on a question. Displayed within the buttons.
    pub point_value: String,
}

#[derive(Deserialize, Debug)]
pub struct PrefilledTestData {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
}
