use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use redis::aio::ConnectionManager;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{PaginationParams, UserCreate, UserResponse, UserUpdate};
use crate::services::user::{
    create_user, delete_user, get_user_by_id, list_users, update_user,
};

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn create_user_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Json(user_data): Json<UserCreate>,
) -> Result<Json<UserResponse>, AppError> {
    create_user(&pool, &redis, user_data).await.map(Json)
}

pub async fn get_user_handler(
    Extension(pool): Extension<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserResponse>, AppError> {
    get_user_by_id(&pool, user_id).await.map(Json)
}

pub async fn list_users_handler(
    Extension(pool): Extension<PgPool>,
    Query(params): Query<PaginationQuery>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100).max(1);

    list_users(&pool, page, page_size).await.map(Json)
}

pub async fn update_user_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Path(user_id): Path<Uuid>,
    Json(update_data): Json<UserUpdate>,
) -> Result<Json<UserResponse>, AppError> {
    update_user(&pool, &redis, user_id, update_data).await.map(Json)
}

pub async fn delete_user_handler(
    State((pool, redis)): State<(PgPool, ConnectionManager)>,
    Path(user_id): Path<Uuid>,
) -> Result<(), AppError> {
    delete_user(&pool, &redis, user_id).await
}