use crate::app::utils::get_env_var;
use lettre::{
    AsyncSmtpTransport, Tokio1Executor,
    message::Mailbox,
    transport::smtp::{PoolConfig, authentication::Credentials},
};
use std::time::Duration;
use tracing::{error, info, warn};
use url::Url;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub is_demo_mode: bool,
    pub server_port: i64,
    pub database_url: String,
    pub redis_url: String,
    pub site_url: Url,
    /// The URL to the terms of service page that Stripe will require people to agree to before
    /// checking out successfully. Can be a link to a Google doc, etc.
    pub tos_url: Url,
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
            .unwrap_or_else(|err| panic!("Failed to parse SITE_URL '{raw_site_url}': {err}"));
        let raw_tos_url = get_env_var("TERMS_OF_SERVICE_URL");
        let tos_url = Url::parse(&raw_tos_url).unwrap_or_else(|err| {
            panic!("Failed to parse TERMS_OF_SERVICE_URL '{raw_tos_url}': {err}")
        });

        Self {
            is_demo_mode,
            server_port,
            database_url,
            redis_url,
            site_url,
            tos_url,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SMTPConfig {
    pub user_email: Mailbox,
    pub mailer: AsyncSmtpTransport<Tokio1Executor>,
}

impl SMTPConfig {
    #[allow(clippy::cognitive_complexity)]
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
                if let Ok(mailbox) = email.parse() {
                    info!(
                        %host, %user, %email,
                        "Email functionality is enabled"
                    );

                    let creds = Credentials::new(user.to_string(), password.to_string());
                    let mailer: AsyncSmtpTransport<Tokio1Executor> = match AsyncSmtpTransport::<
                        Tokio1Executor,
                    >::relay(
                        host
                    ) {
                        Ok(transport) => transport
                            .credentials(creds)
                            .pool_config(
                                PoolConfig::new()
                                    .max_size(10)
                                    .idle_timeout(Duration::from_secs(60)),
                            )
                            .build(),
                        Err(err) => {
                            error!(%err, "Unable to create SMPT Mailer. Proceeding without SMPT functionality.");
                            return None;
                        }
                    };

                    Some(Self {
                        user_email: mailbox,
                        mailer,
                    })
                } else {
                    warn!(
                        "Email functionality disabled. Unable to parse '{email}' into email address."
                    );
                    None
                }
            }
        }
    }
}
