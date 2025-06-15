use std::{collections::HashMap, fs, path::PathBuf};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;



// #######################################################################################################################################################
// #######################################################################################################################################################
// Declare Structs/Enums Used to Define the Test
// #######################################################################################################################################################
// #######################################################################################################################################################

#[derive(Debug, Serialize, Deserialize, Clone)]

pub struct TestDefinitionYaml {
    pub tests: Vec<Test>
}

/// A test object -- can be graded or ungraded, and is used to store the 
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub metadata: Metadata,
    pub tables: Vec<TestTable>,
    pub bonus_items: Option<Vec<BonusItem>>,
}

impl Test {
    /// Load a test definition from a yaml file.
    ///
    /// This function will be run when the app initializes, so we can let it panic. 
    pub fn load(file_path: &PathBuf) -> Self {
        let yaml_string = fs::read_to_string(&file_path).unwrap_or_else(|_| panic!("Couldn't read file '{file_path:?}' to string. This should work..."));
        let test: Self = serde_yaml::from_str(&yaml_string).unwrap_or_else(|_| panic!("Error parsing file `{file_path:?}` to yaml."));
        test.validate().expect("Invalid test definition");
        test

    }

    /// Iterates over each competency scores lists and calculates the max possible score, not including bonus points. 
    fn calculate_max_score(&self) -> i32 {
        self.tables.iter()
            .flat_map(|table| table.sections.iter()) // Flatten the list of lists
            .flat_map(|section| section.competencies.iter()) // Flatten to items to be graded
            .map(|item| {
                item.scores.iter()
                    .map(|score_list| {
                        score_list.iter()
                            .max()
                            .cloned()
                            .unwrap_or(0)
                })
                .sum::<i32>()
            })
            .sum() // Sum the max scores of all items
    }
    

    /// Ensures the score labels are correct, ensures that failing score labels are correct, ensures that antitheses are only present
    /// for single scoring category questions, and ensures that the max score is properly documented in the metadata. 
    /// This violates parse, don't validate, and if this method is not called it is technically possible to have an invalid test
    /// definition, but I'm going to be real, the serde documentation was a huge PITA to figure out the parse don't validate and I'm the
    /// only one using this so just remember to call validate the 2 times you ever deserialize a test from yaml. 
    pub fn validate(&self) -> Result<(), String> {
        for table in &self.tables {
            for section in &table.sections {
                validate_score_labels(&section.competencies, &section.scoring_categories, &self.metadata.test_name)?;

                validate_failing_score_labels(&section.competencies, &section.scoring_categories, &self.metadata.test_name)?;

                validate_antitheses(&section.competencies, &self.metadata.test_name)?;
            }
        }

        if self.calculate_max_score() != self.metadata.max_score {
            return Err(format!(
                "The test metadata for the test named {} indicates a max score of {} when the actual max score (without bonus points) is {}.",
                self.metadata.test_name, self.metadata.max_score, self.calculate_max_score()
            ))
        }
            
        Ok(())
    }

}

/// A table is a collection of sections (groups) of questions/competencies on a test.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TestTable {
    pub test_id: Option<Uuid>,
    pub table_id: Option<Uuid>,
    pub sections: Vec<TestSection>
}

/// A section is a collection of competencies (graded items) that all share the same grade
/// categories
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TestSection {
    pub table_id: Option<Uuid>,
    pub name: String,
    pub scoring_categories: Vec<ScoringCategory>,
    pub competencies: Vec<Competency>,
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
    pub test_id: Option<Uuid>,
    pub test_name: String,
    pub minimum_percent: f32,
    pub max_score: i32,
    pub achieved_score: Option<i32>,
    pub testee: Option<Testee>,
    pub test_date: Option<NaiveDateTime>,
    pub is_graded: Option<()>, // An option being used as a bool. So that serde_yaml parses the data and I don't have to do hella if statements in the askama templates
    pub is_passing: Option<bool>,
    pub proctor: Option<Proctor>,
    pub failure_explanation: Option<Vec<String>>,
    pub config_settings: TestConfig,
}

/// Changes how the test is displayed to the user.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    pub live_grading: bool,
    pub show_point_values: bool,
}

/// A section 
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ScoringCategory {
    pub section_id: Option<Uuid>,
    pub name: String,
    pub values: Vec<String>,
}

/// This is used to hold the score labels that cause a failure
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct FailingScoreLabels {
    pub scoring_category_name: String,
    pub values: Vec<String>, 
}

/// This is used to hold the score label that the proctor gave for a competency during a test.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AchievedScoreLabel {
    pub scoring_category_name: String,
    pub value: String, 
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Competency {
    pub section_id: Option<Uuid>,
    pub name: String,
    pub scores: Vec<Vec<i32>>,
    pub subtext: Option<String>,
    pub failing_score_labels: Option<Vec<FailingScoreLabels>>,
    pub antithesis: Option<String>,
    pub achieved_scores: Option<Vec<i32>>,
    pub achieved_score_labels: Option<Vec<AchievedScoreLabel>>
}

#[derive(Deserialize)]
pub struct PrefilledTestData {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Passing may be failed even if the achieved percent is above the minimum percent if a competency with a failing score label was graded as failing. 
pub struct FullTestSummary {
    pub test_id: Uuid,
    pub test_date: NaiveDateTime,
    pub test_name: String,  // This probably should've been labeled test_type, but I'm lazy here...
    pub proctor: Proctor,
    pub grade_summary: TestGradeSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Passing may be failed even if the achieved percent is above the minimum percent if a competency with a failing score label was graded as failing. 
pub struct TestGradeSummary {
    pub achieved_score: i32,
    pub achieved_percent: f32,
    pub max_score: i32,
    pub minimum_percent: f32,
    pub is_passing: bool,
    pub failure_explanation: Option<Vec<String>>,
}

/// When given the list of GradedItems and the list of HeaderLabels corresponding to a TestSection, will
/// validate that the GradedItems have scores that line up with the number of HeaderLabels in the TestSection. 
/// IE, in the following yaml ensures that there is only one scores list in the graded item named "Body Lead"
/// since there is only one header label, and ensures that the length of that scores list is 5 since there are 
/// 5 values within the header label. 
///   - section_name: "Technique Scoring"
///      scoring_categories:
///      - name: ""
///        values: ["Consistent >90%", "Present 75%", "Occasional 50%", "Lacking 25%", "Missing <10%"]
///      graded_items: 
///        - name: "Body Lead"
///          subtext: "(Week 1)"
///          scores: 
///            - [8, 6, 0, 0, 0]
fn validate_score_labels(graded_items: &[Competency], score_labels: &[ScoringCategory], test_name: &String) -> Result<(), String> {
    
    // Check to ensure that each item has one list of scores per header label.
    let expected_number_of_scores_lists = score_labels.len();
    for item in graded_items {
        if item.scores.len() != expected_number_of_scores_lists {
            return Err(format!(
                "On the test named '{},' graded item '{}' has a number of lists of scores ({}) that does not correspond to the number of scoring categories. ({})",
                test_name, item.name, item.scores.len(), score_labels.len()
            ))
        }
    }

    // Check to ensure that each item's list of scores is the same length as the corresponding list of header labels.
    for (i, score_label) in score_labels.into_iter().enumerate() {
        let expected_number_of_scores = score_label.values.len();
        for item in graded_items {
            if item.scores[i].len() != expected_number_of_scores {
                return Err(format!(
                    "On the test named '{},' the graded item named '{}' has a score list at index {} of length {} that does not correspond to the number of score labels ({}) for the scoring category at index {}.",
                    test_name, item.name, i, item.scores[i].len(), expected_number_of_scores, i
                ))
            }
        }
    }
    Ok(())
}

/// Checks to ensure that all of the failing score labels for the graded items correspond to actual header values.
/// IE, in the following yaml, checks to ensure that the failing score labels for the starter step correspond
/// to actual header section labels and that the values correspond to the values. So it matches the string footwork
/// to footwork and makes sure "Nope" is inside the list of scoring_categories values. 
/// scoring_categories:
/// - name: "Footwork"
///   values: ["Perfect", "Variation?", "Right Concept", "Nope"]
/// - name: "Timing"
///   values: ["On", "Off"]
/// graded_items:
/// - name: "Starter Step"
///   scores: 
///     - [3, 2, 1, 0]
///     - [1, 0]
///   failing_score_labels: 
///     - name: "Footwork"
///       values: ["Nope"]
fn validate_failing_score_labels(graded_items: &[Competency], score_labels: &[ScoringCategory], test_name: &String) -> Result<(), String> {

    // Create a hashmap of the header labels so that we can correspond failing score labels on the graded item to the true header labels
    let mut score_label_hm: HashMap<String, Vec<String>> = HashMap::new();
    for score_label in score_labels {
        if let Some(duplicate_name) = score_label_hm.insert(score_label.name.clone(), score_label.values.clone()) {
            return Err(format!(
                "On the test named '{},' the scoring category name '{:#?}' is not unique within its section.",
                test_name, duplicate_name
            ))
        };
    }

    for item in graded_items {
        match &item.failing_score_labels {
            // Has failing score labels
            Some(labels) => for label in labels {

                match score_label_hm.get(&label.scoring_category_name) {
                    // The failing score label corresponds to a section (ie, the footwork section)
                    Some(valid_failing_score_labels) => for failing_score_label in &label.values {
                        if !valid_failing_score_labels.contains(&failing_score_label) {
                            return Err(format!(
                                "On the test named '{},' the graded item named '{}' has a failing score label '{}' that does not correspond to any of the score labels ({:?}) in the scoring category named '{}'.",
                                test_name, item.name, failing_score_label, valid_failing_score_labels, label.scoring_category_name
                            ))
                        }
                    },
                    // The failing score label does not correspond to a valid section
                    None => return Err(format!(
                        "On the test named '{},' the graded item named '{}' has failing score labels '{:#?}' under the scoring category '{}' that does not correspond to any of the valid scoring category labels ({:?}).",
                        test_name, item.name, label.values, label.scoring_category_name, score_label_hm.keys()
                    ))
                }
            }
            // Does not have failing score labels
            None => continue
        }
    }
    Ok(())
}

/// Ensures that if there is more than one scoring category for an competency (which can be checked by checking the length of the
/// vec of scores) that the item does not have an antithesis. 
fn validate_antitheses(graded_items: &[Competency], test_name: &String) -> Result<(), String> {
    for item in graded_items {
        match &item.antithesis {
            Some(antithesis) => if item.scores.len() > 1 {return Err(format!(
                "On the test named '{},' the competency named '{}' has an antithesis {} which is not supported when there is more than one scoring category for that item.",
                test_name, item.name, antithesis
            ))}
            None => continue
        }
    }
    Ok(())
}



#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Testee {
    pub id: Option<Uuid>,  
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Proctor {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
}

