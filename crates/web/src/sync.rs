use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dainiki_core::parse_date;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        Body, HeaderValue,
        content::Json,
        error::{bad_request, content_too_large, forbidden, not_found, unauthorized},
        path_param,
        request::{Bytes, headers},
        response::Response,
        route,
    },
};

use crate::{
    auth::{self, CurrentUser},
    billing,
};

const MAX_ENTRY_BYTES: usize = 512 * 1024;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const QUOTA_BYTES: i64 = 1024 * 1024 * 1024;

path_param!(entry_id);
path_param!(base_revision: i64, error = bad_request("invalid base revision"));

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub active: bool,
    pub linked: bool,
    pub usage_bytes: i64,
    pub entry_count: i64,
    pub manifest_revision: i64,
}

#[derive(Debug, Deserialize)]
pub struct SyncKeyRequest {
    pub salt: String,
    pub wrapped_key: String,
    pub recovery_wrapped_key: String,
}

#[derive(Debug, Serialize)]
pub struct SyncKeyResponse {
    pub salt: String,
    pub wrapped_key: String,
    pub recovery_wrapped_key: String,
}

#[derive(Debug, Serialize)]
pub struct SyncWriteResponse {
    pub entry_id: String,
    pub revision: i64,
    pub hash: String,
    pub size: i64,
    pub usage_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct ManifestWriteResponse {
    pub revision: i64,
    pub hash: String,
    pub size: i64,
}

#[route(GET "/api/sync/status")]
pub async fn sync_status(cx: &Cx) -> Result<Json<SyncStatus>> {
    let Some(user) = auth::current_user(cx).await? else {
        return Err(unauthorized().into());
    };

    let pool = app_context::<PgPool>(cx);
    let active = billing::has_active_subscription(pool, user.id).await?;
    let linked: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM sync_keys WHERE user_id = $1)")
            .bind(user.id)
            .fetch_one(pool)
            .await?;
    let (usage_bytes, entry_count): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(size), 0)::BIGINT, COUNT(*)::BIGINT
         FROM sync_entries WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_one(pool)
    .await?;
    let manifest_revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT revision FROM sync_manifests WHERE user_id = $1), 0)",
    )
    .bind(user.id)
    .fetch_one(pool)
    .await?;

    Ok(Json(SyncStatus {
        active,
        linked,
        usage_bytes,
        entry_count,
        manifest_revision,
    }))
}

#[route(POST "/api/sync/keys")]
pub async fn setup_sync_keys(
    cx: &Cx,
    Json(input): Json<SyncKeyRequest>,
) -> Result<Json<SyncKeyResponse>> {
    let user = active_user(cx).await?;

    if !valid_encoded(&input.salt, 16)
        || !valid_encoded(&input.wrapped_key, 40)
        || !valid_encoded(&input.recovery_wrapped_key, 40)
    {
        return Err(bad_request("invalid sync key material").into());
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let inserted = sqlx::query(
        "INSERT INTO sync_keys (user_id, kdf_salt, wrapped_key, recovery_wrapped_key, updated_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user.id)
    .bind(&input.salt)
    .bind(&input.wrapped_key)
    .bind(&input.recovery_wrapped_key)
    .bind(now)
    .execute(pool)
    .await?;

    if inserted.rows_affected() == 0 {
        return Err(bad_request("sync is already configured for this account").into());
    }

    Ok(Json(SyncKeyResponse {
        salt: input.salt,
        wrapped_key: input.wrapped_key,
        recovery_wrapped_key: input.recovery_wrapped_key,
    }))
}

#[route(GET "/api/sync/keys")]
pub async fn get_sync_keys(cx: &Cx) -> Result<Json<SyncKeyResponse>> {
    let user = active_user(cx).await?;
    let pool = app_context::<PgPool>(cx);
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT kdf_salt, wrapped_key, recovery_wrapped_key
         FROM sync_keys WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_optional(pool)
    .await?;

    let Some((salt, wrapped_key, recovery_wrapped_key)) = row else {
        return Err(not_found().into());
    };

    Ok(Json(SyncKeyResponse {
        salt,
        wrapped_key,
        recovery_wrapped_key,
    }))
}

#[route(GET "/api/sync/manifest")]
pub async fn get_manifest(cx: &Cx) -> Result<Response> {
    let user = active_user(cx).await?;
    let pool = app_context::<PgPool>(cx);
    let row: Option<(i64, String, i64, Vec<u8>, i64)> = sqlx::query_as(
        "SELECT revision, content_hash, size, ciphertext, updated_at
         FROM sync_manifests WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_optional(pool)
    .await?;

    let Some((revision, hash, size, ciphertext, updated_at)) = row else {
        return Err(not_found().into());
    };

    binary_response(ciphertext, revision, &hash, size, updated_at)
}

#[route(PUT "/api/sync/manifest/{base_revision}")]
pub async fn put_manifest(
    cx: &Cx,
    body: Bytes,
) -> Result<Json<ManifestWriteResponse>> {
    let user = active_user(cx).await?;
    let base = *path_param::<BaseRevision>(cx)?;
    let base = i64::max(0, base);

    if body.len() > MAX_MANIFEST_BYTES {
        return Err(content_too_large().into());
    }

    let hash = required_hash(cx)?;
    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let size = body.len() as i64;
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let current: Option<(i64,)> =
        sqlx::query_as("SELECT revision FROM sync_manifests WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    let current_revision = current.map(|(revision,)| revision).unwrap_or(0);

    if current_revision != base {
        return Err(bad_request("manifest revision conflict").into());
    }

    let revision = current_revision + 1;
    sqlx::query(
        "INSERT INTO sync_manifests
             (user_id, revision, content_hash, size, ciphertext, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (user_id) DO UPDATE SET
             revision = EXCLUDED.revision,
             content_hash = EXCLUDED.content_hash,
             size = EXCLUDED.size,
             ciphertext = EXCLUDED.ciphertext,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(user.id)
    .bind(revision)
    .bind(&hash)
    .bind(size)
    .bind(body.as_ref())
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(ManifestWriteResponse {
        revision,
        hash,
        size,
    }))
}

#[route(GET "/api/sync/entry/{entry_id}")]
pub async fn get_entry(cx: &Cx) -> Result<Response> {
    let user = active_user(cx).await?;
    let entry_id = path_param::<EntryId>(cx).to_string();
    validate_entry_id(&entry_id)?;

    let pool = app_context::<PgPool>(cx);
    let row: Option<(i64, String, i64, Vec<u8>, i64)> = sqlx::query_as(
        "SELECT revision, content_hash, size, ciphertext, updated_at
         FROM sync_entries WHERE user_id = $1 AND entry_id = $2",
    )
    .bind(user.id)
    .bind(&entry_id)
    .fetch_optional(pool)
    .await?;

    let Some((revision, hash, size, ciphertext, updated_at)) = row else {
        return Err(not_found().into());
    };

    binary_response(ciphertext, revision, &hash, size, updated_at)
}

#[route(PUT "/api/sync/entry/{entry_id}/{base_revision}")]
pub async fn put_entry(
    cx: &Cx,
    body: Bytes,
) -> Result<Json<SyncWriteResponse>> {
    let user = active_user(cx).await?;
    let entry_id = path_param::<EntryId>(cx).to_string();
    validate_entry_id(&entry_id)?;
    let base = *path_param::<BaseRevision>(cx)?;
    let base = i64::max(0, base);

    if body.len() > MAX_ENTRY_BYTES {
        return Err(content_too_large().into());
    }

    let hash = required_hash(cx)?;
    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let size = body.len() as i64;
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let current: Option<(i64, i64)> = sqlx::query_as(
        "SELECT revision, size FROM sync_entries
         WHERE user_id = $1 AND entry_id = $2 FOR UPDATE",
    )
    .bind(user.id)
    .bind(&entry_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (current_revision, old_size) = current.unwrap_or((0, 0));

    if current_revision != base {
        return Err(bad_request("entry revision conflict").into());
    }

    let usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0)::BIGINT FROM sync_entries WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let new_usage = usage - old_size + size;

    if new_usage > QUOTA_BYTES {
        return Err(content_too_large().into());
    }

    let revision = current_revision + 1;
    sqlx::query(
        "INSERT INTO sync_entries
             (user_id, entry_id, revision, content_hash, size, ciphertext, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (user_id, entry_id) DO UPDATE SET
             revision = EXCLUDED.revision,
             content_hash = EXCLUDED.content_hash,
             size = EXCLUDED.size,
             ciphertext = EXCLUDED.ciphertext,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(user.id)
    .bind(&entry_id)
    .bind(revision)
    .bind(&hash)
    .bind(size)
    .bind(body.as_ref())
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(SyncWriteResponse {
        entry_id,
        revision,
        hash,
        size,
        usage_bytes: new_usage,
    }))
}

async fn active_user(cx: &Cx) -> Result<CurrentUser> {
    let user = auth::require_user(cx).await?;
    let pool = app_context::<PgPool>(cx);

    if !billing::has_active_subscription(pool, user.id).await? {
        return Err(forbidden().into());
    }

    Ok(user)
}

fn validate_entry_id(entry_id: &str) -> Result<()> {
    if parse_date(entry_id).is_none() {
        return Err(bad_request("entry id must be YYYY-MM-DD").into());
    }

    Ok(())
}

fn required_hash(cx: &Cx) -> Result<String> {
    let hash = headers(cx)
        .get("x-sync-hash")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("missing or invalid x-sync-hash header").into());
    }

    Ok(hash.to_lowercase())
}

fn valid_encoded(value: &str, minimum_bytes: usize) -> bool {
    STANDARD
        .decode(value)
        .map(|bytes| bytes.len() >= minimum_bytes)
        .unwrap_or(false)
}

fn binary_response(
    bytes: Vec<u8>,
    revision: i64,
    hash: &str,
    size: i64,
    updated_at: i64,
) -> Result<Response> {
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        "x-sync-revision",
        HeaderValue::from_str(&revision.to_string())?,
    );
    response.headers_mut().insert(
        "x-sync-hash",
        HeaderValue::from_str(hash)?,
    );
    response.headers_mut().insert(
        "x-sync-size",
        HeaderValue::from_str(&size.to_string())?,
    );
    response.headers_mut().insert(
        "x-sync-updated-at",
        HeaderValue::from_str(&updated_at.to_string())?,
    );

    Ok(response)
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
