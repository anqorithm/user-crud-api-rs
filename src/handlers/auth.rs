use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db::AppState;
use crate::models::{LoginRequest, RefreshRequest, TokenResponse, User, UserCreate};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(user_data): Json<UserCreate>,
) -> impl IntoResponse {
    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&user_data.email)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| tracing::error!("DB error: {:?}", e))
        .ok()
        .flatten();

    if existing.is_some() {
        let response = (
            StatusCode::CONFLICT,
            Json(json!({"error": "User already exists"})),
        );
        return response.into_response();
    }

    let hashed_password = match bcrypt::hash(&user_data.password, bcrypt::DEFAULT_COST) {
        Ok(p) => p,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password hash failed"})),
            );
            return response.into_response();
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
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create user"})),
            );
            return response.into_response();
        }
    };

    let mut conn = state.redis.clone();
    let _: Result<(), _> = conn.del::<_, ()>("users:list:*").await;

    let access_token = match create_access_token(
        user_row.id,
        &user_row.email,
        &state.settings.jwt_secret,
        state.settings.jwt_expiration_hours,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    let refresh_token = match create_refresh_token(
        user_row.id,
        &user_row.email,
        &state.settings.jwt_secret,
        state.settings.jwt_refresh_expiration_days,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(TokenResponse {
            access_token,
            refresh_token,
            token_type: "bearer".to_string(),
        }),
    )
        .into_response()
}

#[axum::debug_handler]
pub async fn login(
    State(state): State<AppState>,
    Json(credentials): Json<LoginRequest>,
) -> impl IntoResponse {
    let user_row: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&credentials.email)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| tracing::error!("DB error: {:?}", e))
        .ok()
        .flatten();

    let user_row = match user_row {
        Some(u) => u,
        None => {
            let response = (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"})),
            );
            return response.into_response();
        }
    };

    let valid = bcrypt::verify(&credentials.password, &user_row.hashed_password).unwrap_or(false);

    if !valid {
        let response = (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"})),
        );
        return response.into_response();
    }

    let access_token = match create_access_token(
        user_row.id,
        &user_row.email,
        &state.settings.jwt_secret,
        state.settings.jwt_expiration_hours,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    let refresh_token = match create_refresh_token(
        user_row.id,
        &user_row.email,
        &state.settings.jwt_secret,
        state.settings.jwt_refresh_expiration_days,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            refresh_token,
            token_type: "bearer".to_string(),
        }),
    )
        .into_response()
}

#[axum::debug_handler]
pub async fn refresh(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> impl IntoResponse {
    let claims = match decode_token(&request.refresh_token, &state.settings.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            let response = (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            );
            return response.into_response();
        }
    };

    let access_token = match create_access_token(
        claims.sub,
        &claims.email,
        &state.settings.jwt_secret,
        state.settings.jwt_expiration_hours,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    let refresh_token = match create_refresh_token(
        claims.sub,
        &claims.email,
        &state.settings.jwt_secret,
        state.settings.jwt_refresh_expiration_days,
    ) {
        Ok(t) => t,
        Err(_) => {
            let response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Token generation failed"})),
            );
            return response.into_response();
        }
    };

    (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            refresh_token,
            token_type: "bearer".to_string(),
        }),
    )
        .into_response()
}

fn create_access_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expiration_hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = (now + Duration::hours(expiration_hours)).timestamp();
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        exp,
        iat: now.timestamp(),
        token_type: "access".to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

fn create_refresh_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expiration_days: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = (now + Duration::days(expiration_days)).timestamp();
    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        exp,
        iat: now.timestamp(),
        token_type: "refresh".to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    let claims = token_data.claims;
    if claims.exp < Utc::now().timestamp() {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::ExpiredSignature,
        ));
    }
    Ok(claims)
}
