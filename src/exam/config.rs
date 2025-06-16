use glob::glob;
use tracing::{error, info};

use crate::exam::models::Test;



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

        let df_1 = polars::df![
            "index" => &[
                "Starter Step",
                "Left Side Pass from Closed",
                "Sugar Tuck",
                "Cutoff Whip",
                "Left Side Pass",
                "Whip",
                "Sugar Push",
                "Spinning Side Pass",
                "Right Side Pass",
                "Basket Whip",
                "Free Spin",
            ],
            "Footwork-.-Perfect" => &[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
            "Footwork-.-Variation?" => &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
            "Footwork-.-Right Concept" => &[2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            "Footwork-.-Nope" => &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            "Timing-.-On" => &[2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
            "Timing-.-Off" => &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ].unwrap();

        let df_2 = polars::df![
            "index" => &[
                "Body Lead",
                "Post",
                "Strong Frame",
                "Closed Connection",
                "Connection Transition",
                "On Time",
                "Move Off Slot",
                "Safe",
            ],
            "-.-Consistent >90%" => &[5, 5, 5, 5, 5, 5, 5, 5],
            "-.-Present 75%" => &[4, 4, 4, 4, 4, 4, 4, 4],
            "-.-Occasional 50%" => &[3, 3, 3, 3, 3, 3, 3, 3],
            "-.-Lacking 25%" => &[2, 2, 2, 2, 2, 2, 2, 2],
            "-.-Missing <10%" => &[1, 1, 1, 1, 1, 1, 1, 1],
        ].unwrap();

        let df_3 = polars::df![
            "index" => &[
                "Body Angle",
            ],
            "-.-Perpendicular" => &[5],
            "-.-Over-Angled" => &[4],
            "-.-Angled" => &[3],
            "-.-Under-Angled" => &[2],
            "-.-Flat" => &[1],
        ].unwrap();


        let df_4 = polars::df![
            "index" => &[
                "Prep",
            ],
            "-.-Overkill" => &[5],
            "-.-Too Strong" => &[4],
            "-.-Adequate" => &[3],
            "-.-Under-Prepped" => &[2],
            "-.-Missing" => &[1],
        ].unwrap();

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
                tables: vec![super::handlers::convert_df_to_test_table(&df_1, &1)],
                dfs: vec![df_1]    
            },

            super::models::TestContainer {
                name: "Technique Scoring".to_string(),
                tables: vec![
                    super::handlers::convert_df_to_test_table(&df_2, &2),
                    super::handlers::convert_df_to_test_table(&df_3, &3),
                    super::handlers::convert_df_to_test_table(&df_4, &4)
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
