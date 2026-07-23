use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use redis::aio::ConnectionManager;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{TaskCreate, TaskResponse, TaskUpdate};
use crate::services::task::{
    create_task, delete_task, get_task_by_id, list_tasks_by_user, update_task,
};
use crate::services::auth::decode_token;

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub email: String,
}

pub async fn create_task_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(task_data): Json<TaskCreate>,
) -> Result<Json<TaskResponse>, AppError> {
    create_task(&pool, &redis, user.user_id, task_data).await.map(Json)
}

pub async fn get_task_handler(
    Extension(pool): Extension<PgPool>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskResponse>, AppError> {
    get_task_by_id(&pool, task_id).await.map(Json)
}

pub async fn list_tasks_handler(
    Extension(pool): Extension<PgPool>,
    Extension(user): Extension<AuthenticatedUser>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Vec<TaskResponse>>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100).max(1);

    list_tasks_by_user(&pool, user.user_id, page, page_size).await.map(Json)
}

pub async fn update_task_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(task_id): Path<Uuid>,
    Json(update_data): Json<TaskUpdate>,
) -> Result<Json<TaskResponse>, AppError> {
    update_task(&pool, &redis, task_id, user.user_id, update_data).await.map(Json)
}

pub async fn delete_task_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(task_id): Path<Uuid>,
) -> Result<(), AppError> {
    delete_task(&pool, &redis, task_id, user.user_id).await
}