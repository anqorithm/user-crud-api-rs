use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use redis::AsyncCommands;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::db::AppState;
use crate::models::{User, UserCreate, UserResponse, UserUpdate};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn create_user_handler(
    State(state): State<AppState>,
    Json(user_data): Json<UserCreate>,
) -> impl IntoResponse {
    let hashed_password = match bcrypt::hash(&user_data.password, bcrypt::DEFAULT_COST) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password hash failed"})),
            )
                .into_response()
        }
    };

    let user_row: Option<User> = sqlx::query_as(
        r#"
        INSERT INTO users (name, email, hashed_password, age)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, email, age, is_active, is_superuser, hashed_password, created_at, updated_at
        "#,
    )
    .bind(&user_data.name)
    .bind(&user_data.email)
    .bind(&hashed_password)
    .bind(user_data.age)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    let user_row = match user_row {
        Some(u) => u,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create user"})),
            )
                .into_response()
        }
    };

    let mut conn = state.redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;

    (StatusCode::CREATED, Json(UserResponse::from(user_row))).into_response()
}

pub async fn get_user_handler(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_row: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| tracing::error!("DB error: {:?}", e))
        .ok()
        .flatten();

    match user_row {
        Some(u) => (StatusCode::OK, Json(UserResponse::from(u))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        )
            .into_response(),
    }
}

pub async fn list_users_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let users: Vec<User> =
        sqlx::query_as("SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| tracing::error!("DB error: {:?}", e))
            .ok()
            .unwrap_or_default();

    (
        StatusCode::OK,
        Json(
            users
                .into_iter()
                .map(UserResponse::from)
                .collect::<Vec<_>>(),
        ),
    )
        .into_response()
}

pub async fn update_user_handler(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(update_data): Json<UserUpdate>,
) -> impl IntoResponse {
    let existing: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| tracing::error!("DB error: {:?}", e))
        .ok()
        .flatten();

    let existing_user = match existing {
        Some(u) => u,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
                .into_response()
        }
    };

    let name = update_data.name.unwrap_or(existing_user.name);
    let email = update_data.email.unwrap_or(existing_user.email);
    let age = update_data.age.or(existing_user.age);
    let is_active = update_data.is_active.unwrap_or(existing_user.is_active);

    let user_row: Option<User> = sqlx::query_as(
        r#"
        UPDATE users SET name = $1, email = $2, age = $3, is_active = $4, updated_at = NOW()
        WHERE id = $5
        RETURNING id, name, email, age, is_active, is_superuser, hashed_password, created_at, updated_at
        "#,
    )
    .bind(&name)
    .bind(&email)
    .bind(age)
    .bind(is_active)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    match user_row {
        Some(u) => {
            let mut conn = state.redis.clone();
            let _: Result<(), _> = conn.del::<_, ()>(&format!("users:{}", user_id)).await;
            let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;
            (StatusCode::OK, Json(UserResponse::from(u))).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        )
            .into_response(),
    }
}

pub async fn delete_user_handler(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() == 0 {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "User not found"})),
                )
                    .into_response()
            } else {
                let mut conn = state.redis.clone();
                let _: Result<(), _> = conn.del::<_, ()>(&format!("users:{}", user_id)).await;
                let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;
                (StatusCode::NO_CONTENT, Json(json!({}))).into_response()
            }
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
            .into_response(),
    }
}
