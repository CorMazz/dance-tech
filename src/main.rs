//! # Main entry point for the Axum web application

#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::multiple_crate_versions)]

use check_in::actors::product_manager_actor_runtime;
use exam::config::ExamConfig;
use tracing::{error, info};
use tracing_subscriber::util::SubscriberInitExt;
mod app;
mod auth;
mod check_in;
mod exam;
use app::config::{AppConfig, SMTPConfig};
use app::router::create_router;
use auth::config::{AuthConfig, GoogleOAuthConfig};
use axum::http::{
    HeaderValue, Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use check_in::config::CheckInConfig;
use dotenv::dotenv;
use oauth2::reqwest;
use redis::Client;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, fmt};

/// The global application state shared across features.
#[allow(dead_code)]
pub struct AppState {
    db: Pool<Postgres>,
    app_config: AppConfig,
    auth_config: AuthConfig,
    check_in_config: CheckInConfig,
    exam_config: ExamConfig,
    redis_client: Client,
    /// Configuration for the SMTP mailing. None if email functionality is not required.
    smtp_config: Option<SMTPConfig>,
    /// Configuration for sign-in with Google OAuth. None if Google sign-in is not required.
    google_oauth_config: Option<GoogleOAuthConfig>,
    /// An HTTP client used to make requests to the Stripe API and Google OAuth endpoints.
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().pretty())
        .init();

    let app_config = AppConfig::init();
    let auth_config = AuthConfig::init();
    let exam_config = ExamConfig::init();

    // 32 because I think the docs said it stores that many by default in memory
    let (product_request_tx, product_request_rx) = mpsc::channel(32);
    let (trigger_update_tx, trigger_update_rx) = mpsc::channel(32);
    let check_in_config = CheckInConfig::init(product_request_tx, trigger_update_tx);

    // For Google OAuth flow and Stripe API requests
    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let smtp_config = SMTPConfig::init();

    let google_oauth_config = GoogleOAuthConfig::init();

    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .connect(&app_config.database_url)
        .await
    {
        Ok(pool) => {
            info!("✅ Connection to the database is successful!");
            pool
        }
        Err(e) => {
            error!("🔥 Failed to connect to the database: {e:?}");
            std::process::exit(1);
        }
    };

    sqlx::migrate!("./migrations").run(&pool).await.expect("Unable to perform database migrations");

    let redis_client = match Client::open(app_config.redis_url.clone()) {
        Ok(client) => {
            info!("✅ Connection to the redis server is successful!");
            client
        }
        Err(e) => {
            error!("🔥 Error connecting to Redis: {e}");
            std::process::exit(1);
        }
    };

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    let app_state = Arc::new(AppState {
        db: pool,
        app_config: app_config.clone(),
        auth_config,
        check_in_config,
        exam_config,
        smtp_config,
        google_oauth_config,
        http_client,
        redis_client,
    });

    let actor_app_state = app_state.clone();
    tokio::spawn(async move {
        product_manager_actor_runtime(product_request_rx, trigger_update_rx, actor_app_state).await;
    });

    let app = create_router(app_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    info!(
        "🚀 Server started successfully on port {}",
        app_config.server_port
    );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", app_config.server_port))
        .await
        .unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}
