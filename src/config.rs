use serde::{Serialize, Deserialize};
use crate::utils::get_env_var;





#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SMTPConfig {
    pub server_host: String,
    pub user_login: String,
    pub user_password: String,
    pub user_email: String,
}

impl SMTPConfig {
    pub fn init() -> Option<SMTPConfig> {
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
                println!("\nEmail functionality disabled since all SMTP environment variables were left blank.");
                None
            }
            ("", _, _, _) => {
                println!("\nEmail functionality disabled: missing SMTP_SERVER_HOST.");
                None
            }
            (_, "", _, _) => {
                println!("\nEmail functionality disabled: missing SMTP_USER_LOGIN.");
                None
            }
            (_, _, "", _) => {
                println!("\nEmail functionality disabled: missing SMTP_USER_PASSWORD.");
                None
            }
            (_, _, _, "") => {
                println!("\nEmail functionality disabled: missing SMTP_USER_EMAIL.");
                None
            }
            (host, user, password, email) => {
                println!(
                    "\nEmail functionality is enabled with the following settings:\n\tServer: {}\n\tUsername: {}\n\tEmail: {}\n",
                    host, user, email
                );

                Some(SMTPConfig {
                    server_host: host.to_string(),
                    user_login: user.to_string(),
                    user_password: password.to_string(),
                    user_email: email.to_string(),
                })
            }
        }
    }
}
