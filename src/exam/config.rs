use std::collections::{BTreeMap, HashMap};

use csv::ReaderBuilder;
use glob::glob;
use tracing::{error, info};

use crate::exam::models::Test;

use super::models::{CsvTestTable, ScoringCategory};

pub fn parse_csv(csv_data: &str) -> CsvTestTable {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_data.as_bytes());

    let headers = rdr.headers().unwrap().iter().map(|h| h.to_string()).collect();
    let mut all_rows = vec![headers];

    for result in rdr.records() {
        let record = result.unwrap();
        let row = record.iter().map(|s| s.to_string()).collect();
        all_rows.push(row);
    }

    CsvTestTable {
        rows: all_rows,
    }
}

pub struct ExamConfig {
    pub tests: Vec<Test>
}

impl ExamConfig {
    /// Glob the `test_definitions/` directory for `.yml` files and read those in as tests.
    ///
    /// Can panic because this runs before the server initializes.
    #[allow(clippy::cognitive_complexity)]
    pub fn init() -> Self {
        // let mut tests: Vec<Test> = Vec::new();
        // let patterns =  ["test_definitions/**/*.yaml", "test_definitions/**/*.yml"];
        //
        // for pattern in patterns {
        //     for entry in glob(pattern).expect("Failed to read glob pattern") {
        //         match entry {
        //             Ok(path) => {
        //                 match std::fs::canonicalize(&path) {
        //                     Ok(abs_path) => {
        //                         info!("Found test definition: {path:?}");
        //                         let test = Test::load(&abs_path);
        //                         test.validate().expect("Invalid test definition:");
        //                         info!("Successfully validated test definition: {path:?}");
        //                         tests.push(test);
        //                     }
        //                     Err(e) => {
        //                         error!(
        //                             "Found file matching pattern but couldn't resolve absolute path: {}\nError: {}",
        //                             path.display(),
        //                             e
        //                         );
        //                     }
        //                 }
        //             }
        //             Err(e) => error!("Glob error: {}", e),
        //         }
        //     }
        // }
        //
        // // Perhaps in the future we'd want this to be able to run without tests
        // assert!(
        //     !tests.is_empty(),
        //     "No tests were read from `test_definitions/`. Make sure .yaml or .yml files exist in that directory.\n\
        //     Current working directory: {}",
        //     std::env::current_dir().map_or_else(|_| "unknown".into(), |p| p.display().to_string())
        // );

        let df_1_csv = r"index,Footwork--~--Perfect,Footwork--~--Variation?,Footwork--~--Right Concept,Footwork--~--Nope,Timing--~--On,Timing--~--Off
Starter Step,4,3,2,1,2,1
Left Side Pass from Closed,4,3,2,1,2,1
Sugar Tuck,4,3,2,1,2,1
Cutoff Whip,4,3,2,1,2,1
Left Side Pass,4,3,2,1,2,1
Whip,4,3,2,1,2,1
Sugar Push,4,3,2,1,2,1
Spinning Side Pass,4,3,2,1,2,1
Right Side Pass,4,3,2,1,2,1
Basket Whip,4,3,2,1,2,1
Free Spin,4,3,2,1,2,1
";

    let df_2_csv = r"index,--~--Consistent >90%,--~--Present 75%,--~--Occasional 50%,--~--Lacking 25%,--~--Missing <10%
Body Lead,5,4,3,2,1
Post,5,4,3,2,1
Strong Frame,5,4,3,2,1
Closed Connection,5,4,3,2,1
Connection Transition,5,4,3,2,1
On Time,5,4,3,2,1
Move Off Slot,5,4,3,2,1
Safe,5,4,3,2,1
";

    let df_3_csv = r"index,--~--Perpendicular,--~--Over-Angled,--~--Angled,--~--Under-Angled,--~--Flat
Body Angle,5,4,3,2,1
";

    let df_4_csv = r"index,--~--Overkill,--~--Too Strong,--~--Adequate,--~--Under-Prepped,--~--Missing
Prep,5,4,3,2,1
";

        let df_1 = parse_csv(df_1_csv);
        let df_2 = parse_csv(df_2_csv);
        let df_3 = parse_csv(df_3_csv);
        let df_4 = parse_csv(df_4_csv);
        let metadata = super::models::Metadata {
            test_name: "Plz Work".to_string(),
            config: super::models::TestConfig {
                live_grading: true,
                show_point_values: true,
            },
            max_score: 69,
            minimum_percent: 0.69,
        };
        let containers = vec![
            super::models::TestContainer {
                name: "Pattern Scoring".to_string(),
                tables: vec![super::handlers::convert_df_to_test_table(&df_1, 0, 0)],
                dfs: vec![df_1]    
            },

            super::models::TestContainer {
                name: "Technique Scoring".to_string(),
                tables: vec![
                    super::handlers::convert_df_to_test_table(&df_2, 1, 0),
                    super::handlers::convert_df_to_test_table(&df_3, 1, 1),
                    super::handlers::convert_df_to_test_table(&df_4, 1, 2)
                ],
                dfs: vec![df_2, df_3, df_4]    
            }
        ];

        let test = Test {
            metadata,
            containers,
            bonus_items: None 
        };

        Self { tests: vec![test] }
    }
}
