//! Utility functions for the application.

/// Get an environment variable or panic if not set.
pub fn get_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| {
        panic!(
            "{var_name} must be set as an environment variable (use an empty string if optional)"
        )
    })
}
