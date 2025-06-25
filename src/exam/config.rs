use glob::glob;
use tracing::{error, info};
use crate::exam::models::Test;
use crate::exam::models::deserialize::TestYaml;

pub struct ExamConfig {
    pub tests: Vec<Test>,
    pub test_names: Vec<String>,
}

impl ExamConfig {
    /// Glob the `test_definitions/` directory for `.yml` files and read those in as tests.
    ///
    /// Can panic because this runs before the server initializes.
    #[allow(clippy::cognitive_complexity)]
    pub fn init() -> Self {
        let mut tests: Vec<Test> = Vec::new();
        let patterns =  ["test_definitions/**/*.yaml", "test_definitions/**/*.yml"];

        for pattern in patterns {
            for entry in glob(pattern).expect("Failed to read glob pattern") {
                match entry {
                    Ok(path) => {
                        match std::fs::canonicalize(&path) {
                            Ok(abs_path) => {
                                info!("Found test definition: {path:?}");
                                let test_yaml = TestYaml::load(&abs_path).expect("Unable to load or parse test");
                                let test = test_yaml.into_test(); 
                                info!("Successfully validated test definition: {path:?}");
                                tests.push(test);
                            }
                            Err(e) => {
                                error!(
                                    "Found file matching pattern but couldn't resolve absolute path: {}\nError: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => error!("Glob error: {}", e),
                }
            }
        }

        // Perhaps in the future we'd want this to be able to run without tests
        assert!(
            !tests.is_empty(),
            "No tests were read from `test_definitions/`. Make sure .yaml or .yml files exist in that directory.\n\
            Current working directory: {}",
            std::env::current_dir().map_or_else(|_| "unknown".into(), |p| p.display().to_string())
        );

        let mut test_names: Vec<String> = Vec::with_capacity(tests.len());
        for test in &tests {
            test_names.push(test.metadata.test_name.clone());
        }

        Self { tests, test_names }
    }
}
