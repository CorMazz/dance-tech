use crate::app::utils::get_env_var;
use serde::{Deserialize, Serialize};
use tracing::info;
use url::Url;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub is_demo_mode: bool,
    pub server_port: i64,
    pub database_url: String,
    pub redis_url: String,
    pub site_url: Url
}

impl AppConfig {
    pub fn init() -> Self {
        let is_demo_mode = get_env_var("DEMO_MODE").parse::<bool>().unwrap_or(false);
        let server_port = get_env_var("SERVER_PORT")
            .parse::<i64>()
            .expect("Server port (ENV_VAR=SERVER_PORT) should be an integer.");
        let database_url = get_env_var("DATABASE_URL");
        let redis_url = get_env_var("REDIS_URL");
        let raw_site_url = get_env_var("SITE_URL");
        let site_url = Url::parse(&raw_site_url)
            .unwrap_or_else(|err| {
                panic!("Failed to parse SITE_URL '{raw_site_url}': {err}")
            });

        Self {
            is_demo_mode,
            server_port,
            database_url,
            redis_url,
            site_url
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SMTPConfig {
    pub server_host: String,
    pub user_login: String,
    pub user_password: String,
    pub user_email: String,
}

impl SMTPConfig {
    pub fn init() -> Option<Self> {
        let server_host = get_env_var("SMTP_SERVER_HOST");
        let user_login = get_env_var("SMTP_USER_LOGIN");
        let user_password = get_env_var("SMTP_USER_PASSWORD");
        let user_email = get_env_var("SMTP_USER_EMAIL");

        match (
            server_host.as_str(),
            user_login.as_str(),
            user_password.as_str(),
            user_email.as_str(),
        ) {
            ("", "", "", "") => {
                info!(
                    "\nEmail functionality disabled since all SMTP environment variables were left blank."
                );
                None
            }
            ("", _, _, _) => {
                info!("\nEmail functionality disabled: missing SMTP_SERVER_HOST.");
                None
            }
            (_, "", _, _) => {
                info!("\nEmail functionality disabled: missing SMTP_USER_LOGIN.");
                None
            }
            (_, _, "", _) => {
                info!("\nEmail functionality disabled: missing SMTP_USER_PASSWORD.");
                None
            }
            (_, _, _, "") => {
                info!("\nEmail functionality disabled: missing SMTP_USER_EMAIL.");
                None
            }
            (host, user, password, email) => {
                info!(
                    "\nEmail functionality is enabled with the following settings:\n\tServer: {host}\n\tUsername: {user}\n\tEmail: {email}\n"
                );

                Some(Self {
                    server_host: host.to_string(),
                    user_login: user.to_string(),
                    user_password: password.to_string(),
                    user_email: email.to_string(),
                })
            }
        }
    }
}
