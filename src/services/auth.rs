use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

pub fn create_access_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expiration_hours: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = (now + Duration::hours(expiration_hours)).timestamp();

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        exp,
        iat: now.timestamp(),
        token_type: "access".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

pub fn create_refresh_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expiration_days: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = (now + Duration::days(expiration_days)).timestamp();

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        exp,
        iat: now.timestamp(),
        token_type: "refresh".to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    let claims = token_data.claims;

    if claims.token_type != "access" {
        return Err(AppError::InvalidToken);
    }

    if claims.exp < Utc::now().timestamp() {
        return Err(AppError::TokenExpired);
    }

    Ok(claims)
}