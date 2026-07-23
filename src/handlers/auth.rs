use axum::{Extension, Json};
use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::config::Settings;
use crate::error::AppError;
use crate::models::{LoginRequest, RefreshRequest, TokenResponse, UserCreate, UserResponse};
use crate::services::auth::{create_access_token, create_refresh_token, verify_token};
use crate::services::user::{create_user, get_user_by_email, verify_password};

pub async fn register(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<ConnectionManager>,
    Extension(settings): Extension<Settings>,
    Json(user_data): Json<UserCreate>,
) -> Result<Json<TokenResponse>, AppError> {
    let user = create_user(&pool, &redis, user_data).await?;

    let access_token = create_access_token(
        user.id,
        &user.email,
        &settings.jwt_secret,
        settings.jwt_expiration_hours,
    )?;

    let refresh_token = create_refresh_token(
        user.id,
        &user.email,
        &settings.jwt_secret,
        settings.jwt_refresh_expiration_days,
    )?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        token_type: "bearer".to_string(),
    }))
}

pub async fn login(
    Extension(pool): Extension<PgPool>,
    Extension(settings): Extension<Settings>,
    Json(credentials): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let user = get_user_by_email(&pool, &credentials.email).await?;

    if !verify_password(&credentials.password, &user.hashed_password)? {
        return Err(AppError::InvalidCredentials);
    }

    let access_token = create_access_token(
        user.id,
        &user.email,
        &settings.jwt_secret,
        settings.jwt_expiration_hours,
    )?;

    let refresh_token = create_refresh_token(
        user.id,
        &user.email,
        &settings.jwt_secret,
        settings.jwt_refresh_expiration_days,
    )?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        token_type: "bearer".to_string(),
    }))
}

pub async fn refresh(
    Extension(pool): Extension<PgPool>,
    Extension(settings): Extension<Settings>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let claims = verify_token(&request.refresh_token, &settings.jwt_secret)?;

    let access_token = create_access_token(
        claims.sub,
        &claims.email,
        &settings.jwt_secret,
        settings.jwt_expiration_hours,
    )?;

    let refresh_token = create_refresh_token(
        claims.sub,
        &claims.email,
        &settings.jwt_secret,
        settings.jwt_refresh_expiration_days,
    )?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        token_type: "bearer".to_string(),
    }))
}