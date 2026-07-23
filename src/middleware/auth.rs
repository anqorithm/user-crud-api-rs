use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::config::Settings;
use crate::error::AppError;
use crate::handlers::tasks::AuthenticatedUser;
use crate::services::auth::decode_token;

pub async fn auth_guard(
    State(settings): State<Settings>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .ok_or(AppError::Unauthorized)?;

    let auth_str = auth_header.to_str().map_err(|_| AppError::Unauthorized)?;

    let token = auth_str.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;

    let claims = decode_token(token, &settings.jwt_secret)?;

    let user = AuthenticatedUser {
        user_id: claims.sub,
        email: claims.email,
    };

    request.extensions_mut().insert(user);

    Ok(next.run(request).await)
}