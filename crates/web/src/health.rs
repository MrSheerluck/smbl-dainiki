use sqlx::PgPool;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::service_unavailable, route},
};

#[route(GET "/api/health")]
pub async fn health() -> Result<&'static str> {
    Ok("ok")
}

#[route(GET "/api/ready")]
pub async fn ready(cx: &Cx) -> Result<&'static str> {
    let pool = app_context::<PgPool>(cx);

    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool).await {
        Ok(_) => Ok("ready"),
        Err(error) => {
            eprintln!("[health] database readiness check failed: {error}");
            Err(service_unavailable(5).into())
        }
    }
}
