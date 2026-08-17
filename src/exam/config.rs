use crate::exam::models::Test;
use crate::exam::models::deserialize::TestYaml;
use glob::glob;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info};

/// In-memory display flags an admin can flip without rewriting YAML.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestDisplayFlag {
    LiveGrading,
    ShowPointValues,
}

struct DisplayFlags {
    live_grading: AtomicBool,
    show_point_values: AtomicBool,
}

pub struct ExamConfig {
    /// YAML-loaded tests. Display flags on these structs stay as the file defined them.
    pub tests: Vec<Test>,
    pub test_names: Vec<String>,
    pub queue_length: usize,
    /// Live grading / show points for this process. Restart reloads YAML into this.
    display: Vec<DisplayFlags>,
}

impl ExamConfig {
    /// Glob the `test_definitions/` directory for `.yml` files and read those in as tests.
    ///
    /// Can panic because this runs before the server initializes.
    #[allow(clippy::cognitive_complexity)]
    pub fn init() -> Self {
        let mut tests: Vec<Test> = Vec::new();
        let patterns = ["test_definitions/**/*.yaml", "test_definitions/**/*.yml"];

        for pattern in patterns {
            for entry in glob(pattern).expect("Failed to read glob pattern") {
                match entry {
                    Ok(path) => match std::fs::canonicalize(&path) {
                        Ok(abs_path) => {
                            info!("Found test definition: {path:?}");
                            let test_yaml =
                                TestYaml::load(&abs_path).expect("Unable to load or parse test");
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
                    },
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

        let queue_length = std::env::var("QUEUE_LENGTH")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<usize>()
            .unwrap_or(15);
        let display = tests
            .iter()
            .map(|test| DisplayFlags {
                live_grading: AtomicBool::new(test.metadata.config.live_grading),
                show_point_values: AtomicBool::new(test.metadata.config.show_point_values),
            })
            .collect();
        Self {
            tests,
            test_names,
            queue_length,
            display,
        }
    }

    /// Clone of a test with this process's live-grading / show-points flags applied.
    pub fn runtime_test(&self, index: usize) -> Option<Test> {
        let over = self.display.get(index)?;
        let mut test = self.tests.get(index)?.clone();
        test.metadata.config.live_grading = over.live_grading.load(Ordering::Relaxed);
        test.metadata.config.show_point_values = over.show_point_values.load(Ordering::Relaxed);
        Some(test)
    }

    /// Every test with this process's display flags. YAML structs are not mutated.
    pub fn runtime_tests(&self) -> Vec<Test> {
        (0..self.tests.len())
            .filter_map(|index| self.runtime_test(index))
            .collect()
    }

    /// Flip one display flag. Returns the test after the change.
    pub fn toggle_display(&self, index: usize, flag: TestDisplayFlag) -> Option<Test> {
        let over = self.display.get(index)?;
        match flag {
            TestDisplayFlag::LiveGrading => {
                over.live_grading.fetch_xor(true, Ordering::Relaxed);
            }
            TestDisplayFlag::ShowPointValues => {
                over.show_point_values.fetch_xor(true, Ordering::Relaxed);
            }
        }
        self.runtime_test(index)
    }
}

#[cfg(test)]
mod tests {
    use super::{ExamConfig, TestDisplayFlag};

    #[test]
    fn toggle_display_is_in_memory_only() {
        let config = ExamConfig::init();
        let yaml_live = config.tests[0].metadata.config.live_grading;
        let yaml_points = config.tests[0].metadata.config.show_point_values;
        assert_eq!(
            config.runtime_test(0).unwrap().metadata.config.live_grading,
            yaml_live
        );

        config
            .toggle_display(0, TestDisplayFlag::LiveGrading)
            .unwrap();
        config
            .toggle_display(0, TestDisplayFlag::ShowPointValues)
            .unwrap();

        let runtime = config.runtime_test(0).unwrap();
        assert_ne!(runtime.metadata.config.live_grading, yaml_live);
        assert_ne!(runtime.metadata.config.show_point_values, yaml_points);
        assert_eq!(config.tests[0].metadata.config.live_grading, yaml_live);
        assert_eq!(
            config.tests[0].metadata.config.show_point_values,
            yaml_points
        );
    }

    #[test]
    fn toggle_display_rejects_unknown_index() {
        let config = ExamConfig::init();
        assert!(
            config
                .toggle_display(config.tests.len(), TestDisplayFlag::LiveGrading)
                .is_none()
        );
    }
}
