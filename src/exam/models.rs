use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tracing::{error, instrument};
use uuid::Uuid;

use crate::auth::models::User;

use super::errors::ExamError;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub metadata: Metadata,
    /// Contains multiple different tables within it. Each table has a new set of headers
    pub containers: Vec<TestContainer>,
    pub bonus_items: Vec<BonusItem>,
}

impl Test {
    #[instrument(skip(self))]
    /// Use the results of the form to mutate the `RadioOption.checked` field to `true` for the
    /// items contained within this test.
    pub fn grade(
        mut self,
        competencies: Vec<(RadioName, RadioValue)>,
        bonus_indices: Vec<usize>,
        proctor_id: Uuid,
    ) -> Result<GradedTest, ExamError> {
        let mut failure_explanations: Vec<FailureExplanation> = Vec::new();
        let mut score = 0;

        // Registry of all expected radio buttons by RadioName to ensure that the form hits all
        // buttons
        let mut competency_registry: HashSet<RadioName> = HashSet::new();

        // --- Reset all radio options to unchecked and build registry ---
        for (container_idx, container) in self.containers.iter_mut().enumerate() {
            for (table_idx, table) in container.tables.iter_mut().enumerate() {
                for (row_idx, row) in table.rows.iter_mut().enumerate() {
                    for (category_idx, button) in row.buttons.iter_mut().enumerate() {
                        for option in &mut button.options {
                            option.checked = false;

                            competency_registry.insert(RadioName {
                                container_index: container_idx,
                                table_index: table_idx,
                                category_index: category_idx,
                                row_index: row_idx,
                            });
                        }
                    }
                }
            }
        }

        for (radio_name, radio_value) in competencies {
            let error_func = |component: &str| {
                error!(
                    "Index out of range for the {component}. `RadioName`: {radio_name:#?}\n`RadioValue`: {radio_value:#?}"
                );
                ExamError::ParseError
            };

            let container = self
                .containers
                .get_mut(radio_name.container_index)
                .ok_or_else(|| error_func("container"))?;

            let table = container
                .tables
                .get_mut(radio_name.table_index)
                .ok_or_else(|| error_func("table"))?;

            let row = table
                .rows
                .get_mut(radio_name.row_index)
                .ok_or_else(|| error_func("row"))?;

            let button = row
                .buttons
                .get_mut(radio_name.category_index)
                .ok_or_else(|| error_func("button"))?;

            let selected_option = button
                .options
                .get_mut(radio_value.label_index)
                .ok_or_else(|| error_func("option"))?;

            score += selected_option.value.point_value;
            selected_option.checked = true;

            if selected_option.value.fails_test {
                // This could be "Sugar Push"
                let competency_name = row.left_label.clone();

                let scoring_category = table
                    .scoring_categories
                    .get(radio_name.category_index)
                    .ok_or_else(|| error_func("scoring category"))?;

                // This could be "Footwork", but it can also be ""
                let scoring_category_name = scoring_category.name.clone();

                // This could be "Perfect"
                let scoring_category_value = scoring_category
                    .values
                    .get(radio_value.label_index)
                    .ok_or_else(|| error_func("scoring category value"))?
                    .clone();

                failure_explanations.push(FailureExplanation::Competency {
                    competency_name,
                    scoring_category_name,
                    scoring_category_value,
                });
            }

            // Theoretically if we made it down here, this component has to exist on the test...
            if !competency_registry.remove(&radio_name) {
                error!(
                    "Attempted to access a component that does not exist on the test:  `RadioName`: {radio_name:#?}\n`RadioValue`: {radio_value:#?}"
                );
                return Err(ExamError::ParseError);
            }
        }

        for bonus_index in bonus_indices {
            let bonus_item = self.bonus_items.get_mut(bonus_index).ok_or_else(|| {
                error!("Bonus index `{bonus_index}` is out of range on this test.");
                ExamError::ParseError
            })?;

            bonus_item.achieved = true;
            score += bonus_item.score;
        }

        if !competency_registry.is_empty() {
            error!(
                "Not all required form inputs were provided. There are ungraded questions remaining.\nUngraded Questions: {competency_registry:#?}"
            );
            return Err(ExamError::ParseError);
        }
        let passing_percent = self.metadata.minimum_percent;
        let max_score = self.metadata.max_score;

        #[allow(clippy::cast_precision_loss)]
        let percent = (score as f32 / max_score as f32) * 100.0;

        if percent < passing_percent {
            failure_explanations.insert(0, FailureExplanation::Score);
        }
        let is_passing = failure_explanations.is_empty();

        let grade = TestGrade {
            max_score: self.metadata.max_score,
            minimum_percent: self.metadata.minimum_percent,
            achieved_score: score,
            achieved_percent: percent,
            is_passing,
            failure_explanations,
        };

        Ok(GradedTest {
            id: Uuid::new_v4(),
            test: self,
            grade,
            proctor_id,
            testee_id: Uuid::new_v4(),
        })
    }
}

/// A bonus item adds bonus points on the test.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BonusItem {
    /// The index of the bonus item in the entire vector of bonus items in the test.
    pub bonus_index: usize,
    /// The name of the bonus item, displayed to the user.
    pub name: String,
    /// The score of the bonus item. Does not penalize the user for not achieving it, as it is
    /// bonus.
    pub score: usize,
    /// If the bonus item was achieved. Is used to display if the item is `checked` on the form.
    pub achieved: bool,
}

/// Metadata contains information about the test needed for the application to work.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub test_name: String,
    /// 100 = 100%
    pub minimum_percent: f32,
    pub max_score: usize,
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
    pub rows: Vec<Vec<String>>,
}

/// A table can also be thought of as a `DataFrame` style structure with a column multi-index to
/// allow for multiple scoring categories within a single table (if that were possible in Polars).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HtmlTestTable {
    pub scoring_categories: Vec<ScoringCategory>,
    pub rows: Vec<TestRow>,
}

/// The text labels for the different scores
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoringCategory {
    /// The name of the concept being scored. IE: 'Footwork'. Can be `""`.
    pub name: String,
    /// The words used to describe what a score means. IE: 'Perfect', 'Right Concept', 'Not So Much'
    pub values: Vec<String>,
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
    pub options: Vec<RadioOption>,
}

/// The `name` field on a radio button is included in the post request when a form is submitted.
/// We can deserialize the keys in the form request into this `RadioName` type. This allows us to
/// correspond the form results to the original test they came from so that we can grade the test.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
#[allow(clippy::struct_field_names)]
pub struct RadioName {
    /// The index of the container on the form that the `RadioButton` belongs to.
    pub container_index: usize,
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
    /// Determine if this is the one that starts checked
    pub checked: bool,
}

/// The `value` field on a radio button is included in the post request when a form is submitted.
/// We can deserialize the values in the form request into this `RadioValue` type. This allows us to
/// correspond the form results to the original test they came from so that we can grade the test.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RadioValue {
    /// The index of the column of the table on the form that this `RadioOption` belongs to.
    pub label_index: usize,
    /// The point value of a specified answer on a question. Displayed within the buttons.
    pub point_value: usize,
    /// If this value causes the whole test to be a failure
    pub fails_test: bool,
}


/// When specific `RadioOptions` fail the test, this struct is created and fed to the HTML to be
/// rendered. There is no `Display` implementation on this because I want to be able to format it
/// nicely within the HTML.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FailureExplanation {
    /// For when a specific competency is failing the test.
    /// "You achieved a value of 'You're Ass' in category 'Footwork' for 'Sugar Push'. This fails
    /// the test."
    Competency {
        /// This could be "Sugar Push"
        competency_name: String,
        /// This could be "Footwork", but it can also be ""
        scoring_category_name: String,
        /// This could be "Perfect"
        scoring_category_value: String,
    },
    /// For when the overall achieved score is too low
    Score,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GradedTest {
    pub id: Uuid,
    pub test: Test,
    pub grade: TestGrade,
    pub proctor_id: Uuid,
    pub testee_id: Uuid,
}

/// Used to show the results of a test to the user
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestGrade {
    pub achieved_score: usize,
    /// 100 = 100%.
    pub achieved_percent: f32,
    /// The minimum percent threshold to pass the test. Repeated in the test `Metadata`. 100 =
    /// 100%.
    pub minimum_percent: f32,
    /// The maximum score possible on the test without bonus points. Repeated in the test
    /// `Metadata`.
    pub max_score: usize,
    pub is_passing: bool,
    pub failure_explanations: Vec<FailureExplanation>,
}

pub mod deserialize {
    //! A series of structs used to deserialize a test from YAML.
    use std::{fs, path::Path};

    use crate::exam::handlers::{convert_df_to_test_table, parse_csv};

    use super::*;
    

    #[derive(Deserialize)]
    pub struct TestYaml {
        metadata: MetadataYaml,
        containers: Vec<ContainerYaml>,
        bonus_items: Vec<BonusItemYaml>,
    }

    #[derive(Deserialize)]
    struct MetadataYaml {
        pub test_name: String,
        // 100 = 100%
        pub minimum_percent: f32,
        pub config: TestConfig
    }

    #[derive(Deserialize)]
    struct ContainerYaml {
        name: String,
        tables: Vec<CsvTableYaml>,
    }

    #[derive(Deserialize)]
    struct CsvTableYaml {
        csv: String,
    }


    #[derive(Deserialize)]
    struct BonusItemYaml {
        pub name: String,
        pub score: usize,
    }

    impl TestYaml {
        /// Load a `TestYaml` from a YAML file at the given path.
        pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ExamError> {
            let contents = fs::read_to_string(path).map_err(|err| ExamError::ReadError(err.to_string()))?;
            let yaml = serde_yaml::from_str::<Self>(&contents).map_err(|err| ExamError::ReadError(err.to_string()))?;
            Ok(yaml)
        }
        /// Convert a `TestYaml` object into a Test object.
        pub fn into_test(self) -> Test {
            let containers: Vec<TestContainer> = self.containers.iter().enumerate().map(|(i, c)| {
                let dfs: Vec<_> = c.tables.iter().map(|t| parse_csv(&t.csv)).collect();
                let tables = dfs.iter().enumerate().map(|(j, df)| convert_df_to_test_table(df, i, j)).collect();
                TestContainer {
                    name: c.name.clone(),
                    tables,
                    dfs,
                }
            }).collect();
               
            // compute max_score by summing max score per button across all tables
            let max_score: usize = containers
                .iter()
                .flat_map(|container| &container.tables)
                .flat_map(|table| &table.rows)
                .flat_map(|row| &row.buttons)
                .map(|button| {
                    button
                        .options
                        .iter()
                        .map(|opt| opt.value.point_value)
                        .max()
                        .unwrap_or(0)
                })
                .sum();

            let metadata = Metadata {
                test_name: self.metadata.test_name, 
                minimum_percent: self.metadata.minimum_percent,
                max_score,
                config: self.metadata.config,
            };

            let bonus_items: Vec<BonusItem> = self.bonus_items
                .into_iter()
                .enumerate()
                .map(|(i, b)| BonusItem {
                    bonus_index: i,
                    name: b.name,
                    score: b.score,
                    achieved: false,
                })
                .collect();

            Test {
                metadata,
                containers,
                bonus_items,
            }
        }
    }
}

/// A struct representing a user in line to take a specific test (given by the `test_index`).
#[derive(Debug, sqlx::FromRow)]
pub struct QueueEntry {
    pub user: User,
    /// There is an invariant assumed that `test_index` will not be invalid
    /// This invariant is maintained by clearing the queue every time the server starts.
    pub test_index: i32,
    pub test_name: String,
}
