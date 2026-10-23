use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::{PgPool, postgres::PgPoolOptions};
use topcoat::Result;

const DEFAULT_DATABASE_URL: &str = "postgres://dainiki:dainiki@127.0.0.1:5432/dainiki";

pub async fn connect() -> Result<PgPool> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
        .bind(now)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM auth_tokens WHERE expires_at <= $1")
        .bind(now)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM email_change_tokens WHERE expires_at <= $1")
        .bind(now)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM billing_events WHERE received_at < $1")
        .bind(now - 60 * 60 * 24 * 90)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM rate_limits WHERE window_start < $1")
        .bind(now - 86_400)
        .execute(&pool)
        .await?;

    Ok(pool)
}
