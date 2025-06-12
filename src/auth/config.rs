use crate::app::{constants::GOOGLE_OAUTH_CALLBACK_PATH, utils::get_env_var};
use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use tracing::info;
use url::Url;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub access_token_private_key: String,
    pub access_token_public_key: String,
    pub access_token_expires_in: String,
    pub access_token_max_age: i64,

    pub refresh_token_private_key: String,
    pub refresh_token_public_key: String,
    pub refresh_token_expires_in: String,
    pub refresh_token_max_age: i64,
}

impl AuthConfig {
    pub fn init() -> Self {
        let access_token_private_key = get_env_var("ACCESS_TOKEN_PRIVATE_KEY");
        let access_token_public_key = get_env_var("ACCESS_TOKEN_PUBLIC_KEY");
        let access_token_expires_in = get_env_var("ACCESS_TOKEN_EXPIRED_IN");
        let access_token_max_age = get_env_var("ACCESS_TOKEN_MAXAGE")
            .parse::<i64>()
            .expect("Access token max age (ENV_VAR=ACCESS_TOKEN_MAXAGE) should be an integer.");

        let refresh_token_private_key = get_env_var("REFRESH_TOKEN_PRIVATE_KEY");
        let refresh_token_public_key = get_env_var("REFRESH_TOKEN_PUBLIC_KEY");
        let refresh_token_expires_in = get_env_var("REFRESH_TOKEN_EXPIRED_IN");
        let refresh_token_max_age = get_env_var("REFRESH_TOKEN_MAXAGE")
            .parse::<i64>()
            .expect("Refresh token max age (ENV_VAR=REFRESH_TOKEN_MAXAGE) should be an integer.");

        Self {
            access_token_private_key,
            access_token_public_key,
            access_token_expires_in,
            access_token_max_age,
            refresh_token_private_key,
            refresh_token_public_key,
            refresh_token_expires_in,
            refresh_token_max_age,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoogleOAuthConfig {
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
    pub auth_uri: AuthUrl,
    pub token_uri: TokenUrl,
    pub redirect_uri: RedirectUrl,
}
impl GoogleOAuthConfig {
    #[allow(clippy::cognitive_complexity)]
    pub fn init() -> Option<Self> {
        let client_id = get_env_var("GOOGLE_OAUTH_CLIENT_ID");
        let client_secret = get_env_var("GOOGLE_OAUTH_CLIENT_SECRET");
        let auth_uri = get_env_var("GOOGLE_OAUTH_AUTH_URI");
        let token_uri = get_env_var("GOOGLE_OAUTH_TOKEN_URI");
        let site_url = &get_env_var("SITE_URL");
        let mut redirect_uri = Url::parse(site_url)
            .unwrap_or_else(|err| panic!("Failed to parse SITE_URL '{site_url}': {err}"));
        redirect_uri.set_path(GOOGLE_OAUTH_CALLBACK_PATH);

        match (
            client_id.as_str(),
            client_secret.as_str(),
            auth_uri.as_str(),
            token_uri.as_str(),
        ) {
            ("", "", "", "") => {
                info!(
                    "\nGoogle OAuth functionality disabled since all environment variables were left blank."
                );
                None
            }
            ("", _, _, _) => {
                info!("\nGoogle OAuth functionality disabled: missing GOOGLE_OAUTH_CLIENT_ID.");
                None
            }
            (_, "", _, _) => {
                info!("\nGoogle OAuth functionality disabled: missing GOOGLE_OAUTH_CLIENT_SECRET.");
                None
            }
            (_, _, "", _) => {
                info!("\nGoogle OAuth functionality disabled: missing GOOGLE_OAUTH_AUTH_URI.");
                None
            }
            (_, _, _, "") => {
                info!("\nGoogle OAuth functionality disabled: missing GOOGLE_OAUTH_TOKEN_URI.");
                None
            }
            (client_id, client_secret, auth_uri, token_uri) => {
                info!("✅ Google OAuth functionality is enabled.",);

                Some(Self {
                    client_id: ClientId::new(client_id.to_string()),
                    client_secret: ClientSecret::new(client_secret.to_string()),
                    auth_uri: AuthUrl::new(auth_uri.to_string())
                        .expect("Unable to parse GOOGLE_OAUTH_AUTH_URI."),
                    token_uri: TokenUrl::new(token_uri.to_string())
                        .expect("Unable to parse GOOGLE_OAUTH_TOKEN_URI."),
                    redirect_uri: RedirectUrl::new(redirect_uri.to_string())
                        .expect("Unable to parse GOOGLE_OAUTH_REDIRECT_URI."),
                })
            }
        }
    }
}
