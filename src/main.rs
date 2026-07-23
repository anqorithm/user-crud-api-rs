mod config;
mod db;
mod error;
mod handlers;
mod models;

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber;

use crate::config::Settings;
use crate::handlers::{auth, tasks, users};
use crate::db::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let settings = Settings::new().unwrap_or_default();

    let pool = db::create_pool(&settings.database_url).await?;
    db::init_db(&pool).await?;

    let redis_client = redis::Client::open(settings.redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;

    let state = AppState {
        pool,
        redis: redis_conn,
        settings: settings.clone(),
    };

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/users", get(users::list_users_handler))
        .route("/api/v1/users/:id", get(users::get_user_handler))
        .route("/api/v1/users", post(users::create_user_handler))
        .route("/api/v1/users/:id", patch(users::update_user_handler))
        .route("/api/v1/users/:id", delete(users::delete_user_handler))
        .route("/api/v1/tasks", get(tasks::list_tasks_handler))
        .route("/api/v1/tasks", post(tasks::create_task_handler))
        .route("/api/v1/tasks/:id", get(tasks::get_task_handler))
        .route("/api/v1/tasks/:id", patch(tasks::update_task_handler))
        .route("/api/v1/tasks/:id", delete(tasks::delete_task_handler))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}