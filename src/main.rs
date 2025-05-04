//! # Main entry point for the Axum web application

#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::multiple_crate_versions)]

mod app;
mod auth;

use app::config::{AppConfig, SMTPConfig};
use auth::config::{AuthConfig, GoogleOAuthConfig};
use lettre::transport::smtp::PoolConfig;
use lettre::{AsyncSmtpTransport, Tokio1Executor, transport::smtp::authentication::Credentials};
use oauth2::reqwest;
use std::{sync::Arc, time::Duration};

use app::router::create_router;
use axum::http::{
    HeaderValue, Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use dotenv::dotenv;
use redis::Client;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use tower_http::cors::CorsLayer;

/// The global application state shared across features.
#[allow(dead_code)]
pub struct AppState {
    db: Pool<Postgres>,
    app_config: AppConfig,
    auth_config: AuthConfig,
    redis_client: Client,
    smtp_config: Option<SMTPConfig>,
    smtp_mailer: Option<AsyncSmtpTransport<Tokio1Executor>>,
    google_oauth_config: Option<GoogleOAuthConfig>,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let app_config = AppConfig::init();
    let auth_config = AuthConfig::init();

    let smtp_config = SMTPConfig::init();
    let smtp_mailer: Option<AsyncSmtpTransport<Tokio1Executor>> =
        smtp_config.as_ref().and_then(|config| {
            let creds = Credentials::new(config.user_login.clone(), config.user_password.clone());

            match AsyncSmtpTransport::<Tokio1Executor>::relay(&config.server_host) {
                Ok(transport) => Some(
                    transport
                        .credentials(creds)
                        .pool_config(
                            PoolConfig::new()
                                .max_size(10)
                                .idle_timeout(Duration::from_secs(60)),
                        )
                        .build(),
                ),
                Err(e) => {
                    eprintln!("Error: Unable to connect to email server: {e}");
                    None
                }
            }
        });

    let google_oauth_config = GoogleOAuthConfig::init();

    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .connect(&app_config.database_url)
        .await
    {
        Ok(pool) => {
            println!("✅ Connection to the database is successful!");
            pool
        }
        Err(e) => {
            println!("🔥 Failed to connect to the database: {e:?}");
            std::process::exit(1);
        }
    };

    let redis_client = match Client::open(app_config.redis_url.clone()) {
        Ok(client) => {
            println!("✅ Connection to the redis server is successful!");
            client
        }
        Err(e) => {
            println!("🔥 Error connecting to Redis: {e}");
            std::process::exit(1);
        }
    };

    // For Google OAuth flow
    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let app = create_router(Arc::new(AppState {
        db: pool.clone(),
        app_config: app_config.clone(),
        auth_config: auth_config.clone(),
        smtp_config,
        smtp_mailer,
        google_oauth_config,
        http_client,
        redis_client: redis_client.clone(),
    }))
    .layer(cors);

    println!(
        "🚀 Server started successfully on port {}",
        app_config.server_port
    );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", app_config.server_port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
