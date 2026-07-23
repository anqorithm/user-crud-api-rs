use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{Task, TaskCreate, TaskResponse, TaskUpdate};

pub async fn create_task(
    pool: &PgPool,
    redis: &redis::aio::ConnectionManager,
    user_id: Uuid,
    task_data: TaskCreate,
) -> Result<TaskResponse, AppError> {
    let priority = task_data.priority.unwrap_or(1);

    let task: Task = sqlx::query_as(
        r#"
        INSERT INTO tasks (title, description, priority, user_id)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(&task_data.title)
    .bind(&task_data.description)
    .bind(priority)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:user:{}", user_id)).await;

    Ok(task.into())
}

pub async fn get_task_by_id(pool: &PgPool, id: Uuid) -> Result<TaskResponse, AppError> {
    let task: Task = sqlx::query_as(
        "SELECT * FROM tasks WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::TaskNotFound)?;

    Ok(task.into())
}

pub async fn list_tasks_by_user(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    page_size: i64,
) -> Result<Vec<TaskResponse>, AppError> {
    let offset = (page - 1) * page_size;

    let tasks: Vec<Task> = sqlx::query_as(
        "SELECT * FROM tasks WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(tasks.into_iter().map(|t| t.into()).collect())
}

pub async fn update_task(
    pool: &PgPool,
    redis: &redis::aio::ConnectionManager,
    id: Uuid,
    user_id: Uuid,
    update_data: TaskUpdate,
) -> Result<TaskResponse, AppError> {
    let existing: Task = sqlx::query_as(
        "SELECT * FROM tasks WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::TaskNotFound)?;

    let title = update_data.title.unwrap_or(existing.title);
    let description = update_data.description.or(existing.description);
    let priority = update_data.priority.unwrap_or(existing.priority);
    let completed = update_data.completed.unwrap_or(existing.completed);

    let task: Task = sqlx::query_as(
        r#"
        UPDATE tasks SET title = $1, description = $2, priority = $3, completed = $4, updated_at = NOW()
        WHERE id = $5 AND user_id = $6
        RETURNING *
        "#,
    )
    .bind(&title)
    .bind(&description)
    .bind(priority)
    .bind(completed)
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:{}", id)).await;
    let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:user:{}", user_id)).await;

    Ok(task.into())
}

pub async fn delete_task(
    pool: &PgPool,
    redis: &redis::aio::ConnectionManager,
    id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::TaskNotFound);
    }

    let mut conn = redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:{}", id)).await;
    let _: Result<(), _> = conn.del::<_, ()>(&format!("tasks:user:{}", user_id)).await;

    Ok(())
}