//! Utility functions for the application.

/// Get an environment variable or panic if not set.
pub fn get_env_var(var_name: &str) -> String {
    std::env::var(var_name)
        .expect(&format!("{var_name} must be set as an environment variable (use an empty string if optional)"))
}