//! A module for constants which need to be used in multiple places.
//!
//! This is to ensure that we follow the principle of One Absolute Truth and do not duplicate state
//! which can become out of date as development continues.
//! TODO: Put all environment variables into this file within a module
//! TODO: Put all path fragments into this file within a module, and make the templates accept the
//! paths
pub const GOOGLE_OAUTH_CALLBACK_PATH: &str = "/auth/google/callback";
pub const STRIPE_SUCCESS_CALLBACK_PATH: &str = "/stripe/success";
pub const CHECK_IN_PATH: &str = "/check-in";

