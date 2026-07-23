use axum::{
    extract::{Path, Query, State},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use redis::AsyncCommands;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::db::AppState;
use crate::models::{TaskCreate, TaskUpdate, Task, TaskResponse};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn create_task_handler(
    State(state): State<AppState>,
    Json(task_data): Json<TaskCreate>,
) -> impl IntoResponse {
    let user_id = Uuid::new_v4();
    let priority = task_data.priority.unwrap_or(1);

    let task_row: Option<Task> = sqlx::query_as(
        r#"
        INSERT INTO tasks (title, description, priority, user_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, title, description, priority, completed, user_id, created_at, updated_at
        "#,
    )
    .bind(&task_data.title)
    .bind(&task_data.description)
    .bind(priority)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    match task_row {
        Some(t) => {
            let mut conn = state.redis.clone();
            let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:user:{}", user_id)).await;
            (StatusCode::CREATED, Json(TaskResponse::from(t))).into_response()
        }
        None => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create task"}))).into_response(),
    }
}

pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    let task_row: Option<Task> = sqlx::query_as(
        "SELECT * FROM tasks WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    match task_row {
        Some(t) => (StatusCode::OK, Json(TaskResponse::from(t))).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Task not found"}))).into_response(),
    }
}

pub async fn list_tasks_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100).max(1);
    let offset = (page - 1) * page_size;

    let tasks: Vec<Task> = sqlx::query_as(
        "SELECT * FROM tasks ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().unwrap_or_default();

    (StatusCode::OK, Json(tasks.into_iter().map(|task| TaskResponse::from(task)).collect::<Vec<_>>())).into_response()
}

pub async fn update_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
    Json(update_data): Json<TaskUpdate>,
) -> impl IntoResponse {
    let existing: Option<Task> = sqlx::query_as(
        "SELECT * FROM tasks WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    let existing_task = match existing {
        Some(t) => t,
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Task not found"}))).into_response(),
    };

    let title = update_data.title.unwrap_or(existing_task.title);
    let description = update_data.description.or(existing_task.description);
    let priority = update_data.priority.unwrap_or(existing_task.priority);
    let completed = update_data.completed.unwrap_or(existing_task.completed);

    let task_row: Option<Task> = sqlx::query_as(
        r#"
        UPDATE tasks SET title = $1, description = $2, priority = $3, completed = $4, updated_at = NOW()
        WHERE id = $5
        RETURNING id, title, description, priority, completed, user_id, created_at, updated_at
        "#,
    )
    .bind(&title)
    .bind(&description)
    .bind(priority)
    .bind(completed)
    .bind(task_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| tracing::error!("DB error: {:?}", e)).ok().flatten();

    match task_row {
        Some(t) => {
            let mut conn = state.redis.clone();
            let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:{}", task_id)).await;
            (StatusCode::OK, Json(TaskResponse::from(t))).into_response()
        }
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Task not found"}))).into_response(),
    }
}

pub async fn delete_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(task_id)
        .execute(&state.pool)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() == 0 {
                (StatusCode::NOT_FOUND, Json(json!({"error": "Task not found"}))).into_response()
            } else {
                let mut conn = state.redis.clone();
                let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:{}", task_id)).await;
                (StatusCode::NO_CONTENT, Json(json!({}))).into_response()
            }
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response(),
    }
}