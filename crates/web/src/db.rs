use std::time::Duration;

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

    Ok(pool)
}
