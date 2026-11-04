use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub jwt_refresh_expiration_days: i64,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("APP").separator("__"))
            .build()?;

        config.try_deserialize()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@localhost:5432/users_db".into(),
            redis_url: "redis://localhost:6379".into(),
            jwt_secret: "your-super-secret-key-change-in-production".into(),
            jwt_expiration_hours: 24,
            jwt_refresh_expiration_days: 7,
        }
    }
}
