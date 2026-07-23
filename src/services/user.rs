use bcrypt::{hash, verify, DEFAULT_COST};
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{User, UserCreate, UserResponse, UserUpdate};

pub async fn create_user(pool: &PgPool, redis: &redis::aio::ConnectionManager, user_data: UserCreate) -> Result<UserResponse, AppError> {
    let existing: Option<User> = sqlx::query_as(
        "SELECT * FROM users WHERE email = $1",
    )
    .bind(&user_data.email)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::UserAlreadyExists);
    }

    let hashed_password = hash(&user_data.password, DEFAULT_COST)
        .map_err(|_| AppError::InternalError)?;

    let user: User = sqlx::query_as(
        r#"
        INSERT INTO users (name, email, hashed_password, age)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(&user_data.name)
    .bind(&user_data.email)
    .bind(&hashed_password)
    .bind(&user_data.age)
    .fetch_one(pool)
    .await?;

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;

    Ok(user.into())
}

pub async fn get_user_by_id(pool: &PgPool, id: Uuid) -> Result<UserResponse, AppError> {
    let user: User = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::UserNotFound)?;

    Ok(user.into())
}

pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<User, AppError> {
    let user: User = sqlx::query_as(
        "SELECT * FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::UserNotFound)?;

    Ok(user)
}

pub async fn list_users(pool: &PgPool, page: i64, page_size: i64) -> Result<Vec<UserResponse>, AppError> {
    let offset = (page - 1) * page_size;

    let users: Vec<User> = sqlx::query_as(
        "SELECT * FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(users.into_iter().map(|u| u.into()).collect())
}

pub async fn update_user(
    pool: &PgPool,
    redis: &redis::aio::ConnectionManager,
    id: Uuid,
    update_data: UserUpdate,
) -> Result<UserResponse, AppError> {
    let existing: Option<User> = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::UserNotFound)?;

    let name = update_data.name.unwrap_or(existing.name);
    let email = update_data.email.unwrap_or(existing.email);
    let age = update_data.age.or(existing.age);
    let is_active = update_data.is_active.unwrap_or(existing.is_active);

    let user: User = sqlx::query_as(
        r#"
        UPDATE users SET name = $1, email = $2, age = $3, is_active = $4, updated_at = NOW()
        WHERE id = $5
        RETURNING *
        "#,
    )
    .bind(&name)
    .bind(&email)
    .bind(age)
    .bind(is_active)
    .bind(id)
    .fetch_one(pool)
    .await?;

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>(&format!("users:{}", id)).await;
    let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;

    Ok(user.into())
}

pub async fn delete_user(pool: &PgPool, redis: &redis::aio::ConnectionManager, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::UserNotFound);
    }

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>(&format!("users:{}", id)).await;
    let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;

    Ok(())
}

pub fn verify_password(password: &str, hashed: &str) -> Result<bool, AppError> {
    verify(password, hashed).map_err(|_| AppError::InternalError)
}