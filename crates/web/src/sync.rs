use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dainiki_core::parse_date;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Json,
        error::{
            bad_request, content_too_large, forbidden, internal_server_error, not_found,
            service_unavailable, too_many_requests, unauthorized,
        },
        path_param,
        request::{Bytes, headers},
        route,
    },
};

use crate::{
    auth::{self, CurrentUser},
    billing,
    sync_storage::SyncStorage,
};

const MAX_ENTRY_BYTES: i64 = 512 * 1024;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const QUOTA_BYTES: i64 = 1024 * 1024 * 1024;
const UPLOAD_TTL_SECONDS: i64 = 30 * 60;

path_param!(entry_id);
path_param!(base_revision: i64, error = bad_request("invalid base revision"));

#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub active: bool,
    pub linked: bool,
    pub storage_ready: bool,
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

#[derive(Debug, Serialize)]
pub struct ObjectUrl {
    pub url: String,
    pub revision: i64,
    pub hash: String,
    pub size: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct PrepareUpload {
    pub entry_id: String,
    pub base_revision: i64,
    pub size: i64,
    pub hash: String,
}

#[derive(Debug, Serialize)]
pub struct PreparedUpload {
    pub upload_id: i64,
    pub url: String,
    pub revision: i64,
}

#[derive(Debug, Deserialize)]
pub struct CommitUpload {
    pub upload_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct DeleteEntry {
    pub entry_id: String,
    pub base_revision: i64,
}

#[route(GET "/api/sync/status")]
pub async fn sync_status(cx: &Cx) -> Result<Json<SyncStatus>> {
    let Some(user) = auth::current_user(cx).await? else {
        return Err(unauthorized().into());
    };

    let pool = app_context::<PgPool>(cx);
    let storage = app_context::<SyncStorage>(cx);
    let active = billing::has_active_subscription(pool, user.id).await?;
    let linked: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM sync_keys WHERE user_id = $1)")
            .bind(user.id)
            .fetch_one(pool)
            .await?;
    let (usage_bytes, entry_count): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN deleted THEN 0 ELSE size END), 0)::BIGINT,
                COUNT(*) FILTER (WHERE NOT deleted)::BIGINT
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
        storage_ready: storage.enabled(),
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
    require_storage(cx)?;
    rate_limit(cx, user.id, "keys", 10).await?;

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
pub async fn get_manifest(cx: &Cx) -> Result<Json<ObjectUrl>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "manifest-read", 600).await?;
    let pool = app_context::<PgPool>(cx);
    let row: Option<(i64, String, i64, String, i64)> = sqlx::query_as(
        "SELECT revision, content_hash, size, object_key, updated_at
         FROM sync_manifests WHERE user_id = $1 AND object_key IS NOT NULL",
    )
    .bind(user.id)
    .fetch_optional(pool)
    .await?;

    let Some((revision, hash, size, object_key, updated_at)) = row else {
        return Err(not_found().into());
    };

    let url = storage
        .presign_get(&object_key)
        .await
        .map_err(storage_error)?;

    Ok(Json(ObjectUrl {
        url,
        revision,
        hash,
        size,
        updated_at,
    }))
}

#[route(PUT "/api/sync/manifest/{base_revision}")]
pub async fn put_manifest(cx: &Cx, body: Bytes) -> Result<Json<ManifestWriteResponse>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "manifest-write", 600).await?;
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

    let current: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT revision, object_key FROM sync_manifests WHERE user_id = $1")
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    let (current_revision, old_key) = current.unwrap_or((0, None));

    if current_revision != base {
        return Err(bad_request("manifest revision conflict").into());
    }

    let revision = current_revision + 1;
    let object_key = format!("u/{}/manifest/{revision}", user.id);
    storage
        .put(&object_key, body.as_ref().to_vec())
        .await
        .map_err(storage_error)?;

    sqlx::query(
        "INSERT INTO sync_manifests
             (user_id, revision, content_hash, size, ciphertext, object_key, updated_at)
         VALUES ($1, $2, $3, $4, NULL, $5, $6)
         ON CONFLICT (user_id) DO UPDATE SET
             revision = EXCLUDED.revision,
             content_hash = EXCLUDED.content_hash,
             size = EXCLUDED.size,
             ciphertext = NULL,
             object_key = EXCLUDED.object_key,
             updated_at = EXCLUDED.updated_at",
    )
    .bind(user.id)
    .bind(revision)
    .bind(&hash)
    .bind(size)
    .bind(&object_key)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    if let Some(old_key) = old_key {
        if old_key != object_key {
            if let Err(error) = storage.delete(&old_key).await {
                eprintln!("[sync] could not delete old manifest object: {error}");
            }
        }
    }

    Ok(Json(ManifestWriteResponse {
        revision,
        hash,
        size,
    }))
}

#[route(GET "/api/sync/entry/{entry_id}")]
pub async fn get_entry(cx: &Cx) -> Result<Json<ObjectUrl>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "entry-read", 10_000).await?;
    let entry_id = path_param::<EntryId>(cx).to_string();
    validate_entry_id(&entry_id)?;

    let pool = app_context::<PgPool>(cx);
    let row: Option<(i64, String, i64, String, i64)> = sqlx::query_as(
        "SELECT revision, content_hash, size, object_key, updated_at
         FROM sync_entries
         WHERE user_id = $1 AND entry_id = $2
           AND deleted = FALSE AND object_key IS NOT NULL",
    )
    .bind(user.id)
    .bind(&entry_id)
    .fetch_optional(pool)
    .await?;

    let Some((revision, hash, size, object_key, updated_at)) = row else {
        return Err(not_found().into());
    };

    let url = storage
        .presign_get(&object_key)
        .await
        .map_err(storage_error)?;

    Ok(Json(ObjectUrl {
        url,
        revision,
        hash,
        size,
        updated_at,
    }))
}

#[route(POST "/api/sync/entries/prepare")]
pub async fn prepare_entry_upload(
    cx: &Cx,
    Json(input): Json<PrepareUpload>,
) -> Result<Json<PreparedUpload>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "entry-prepare", 3_000).await?;
    validate_entry_id(&input.entry_id)?;
    validate_hash(&input.hash)?;

    if input.base_revision < 0 {
        return Err(bad_request("base revision cannot be negative").into());
    }

    if !(0..=MAX_ENTRY_BYTES).contains(&input.size) {
        return Err(content_too_large().into());
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let cutoff = now - UPLOAD_TTL_SECONDS;
    cleanup_expired_uploads(pool, storage, user.id, cutoff).await?;

    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let current: Option<(i64, i64)> = sqlx::query_as(
        "SELECT revision, size FROM sync_entries
         WHERE user_id = $1 AND entry_id = $2",
    )
    .bind(user.id)
    .bind(&input.entry_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (current_revision, old_size) = current.unwrap_or((0, 0));

    if current_revision != input.base_revision {
        return Err(bad_request("entry revision conflict").into());
    }

    let usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN deleted THEN 0 ELSE size END), 0)::BIGINT
         FROM sync_entries WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let pending: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0)::BIGINT FROM sync_uploads
         WHERE user_id = $1 AND committed_at IS NULL AND created_at >= $2",
    )
    .bind(user.id)
    .bind(cutoff)
    .fetch_one(&mut *tx)
    .await?;

    if usage - old_size + pending + input.size > QUOTA_BYTES {
        return Err(content_too_large().into());
    }

    let revision = current_revision + 1;
    let object_key = format!("u/{}/e/{entry_id}/{revision}", user.id, entry_id = input.entry_id);
    let upload_id: i64 = sqlx::query_scalar(
        "INSERT INTO sync_uploads
             (user_id, entry_id, base_revision, revision, object_key, content_hash, size, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(user.id)
    .bind(&input.entry_id)
    .bind(input.base_revision)
    .bind(revision)
    .bind(&object_key)
    .bind(&input.hash)
    .bind(input.size)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let url = storage
        .presign_put(&object_key)
        .await
        .map_err(storage_error)?;

    Ok(Json(PreparedUpload {
        upload_id,
        url,
        revision,
    }))
}

#[route(POST "/api/sync/entries/commit")]
pub async fn commit_entry_upload(
    cx: &Cx,
    Json(input): Json<CommitUpload>,
) -> Result<Json<SyncWriteResponse>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "entry-commit", 3_000).await?;
    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let upload: Option<(String, i64, i64, String, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT entry_id, base_revision, revision, object_key, content_hash, size, committed_at
         FROM sync_uploads WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(input.upload_id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((entry_id, base_revision, revision, object_key, hash, size, committed_at)) = upload
    else {
        return Err(bad_request("upload was not found").into());
    };

    if committed_at.is_some() {
        return Err(bad_request("upload was already committed").into());
    }

    let current: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT revision, size, COALESCE(object_key, '') FROM sync_entries
         WHERE user_id = $1 AND entry_id = $2",
    )
    .bind(user.id)
    .bind(&entry_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (current_revision, old_size, old_key) = current.unwrap_or((0, 0, String::new()));

    if current_revision != base_revision {
        return Err(bad_request("entry revision conflict").into());
    }

    let actual_size = storage.size(&object_key).await.map_err(storage_error)?;
    if actual_size != size {
        return Err(bad_request("uploaded object has the wrong size").into());
    }

    let usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN deleted THEN 0 ELSE size END), 0)::BIGINT
         FROM sync_entries WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let new_usage = usage - old_size + size;

    if new_usage > QUOTA_BYTES {
        return Err(content_too_large().into());
    }

    sqlx::query(
        "INSERT INTO sync_entries
             (user_id, entry_id, revision, content_hash, size, ciphertext, object_key, updated_at)
         VALUES ($1, $2, $3, $4, $5, NULL, $6, $7)
         ON CONFLICT (user_id, entry_id) DO UPDATE SET
             revision = EXCLUDED.revision,
             content_hash = EXCLUDED.content_hash,
             size = EXCLUDED.size,
             ciphertext = NULL,
             object_key = EXCLUDED.object_key,
             updated_at = EXCLUDED.updated_at,
             deleted = FALSE",
    )
    .bind(user.id)
    .bind(&entry_id)
    .bind(revision)
    .bind(&hash)
    .bind(size)
    .bind(&object_key)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE sync_uploads SET committed_at = $1 WHERE id = $2")
        .bind(now)
        .bind(input.upload_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    if !old_key.is_empty() && old_key != object_key {
        if let Err(error) = storage.delete(&old_key).await {
            eprintln!("[sync] could not delete old entry object: {error}");
        }
    }

    Ok(Json(SyncWriteResponse {
        entry_id,
        revision,
        hash,
        size,
        usage_bytes: new_usage,
    }))
}

#[route(POST "/api/sync/entries/delete")]
pub async fn delete_entry(
    cx: &Cx,
    Json(input): Json<DeleteEntry>,
) -> Result<Json<SyncWriteResponse>> {
    let user = active_user(cx).await?;
    let storage = require_storage(cx)?;
    rate_limit(cx, user.id, "entry-delete", 600).await?;
    validate_entry_id(&input.entry_id)?;

    if input.base_revision < 0 {
        return Err(bad_request("base revision cannot be negative").into());
    }

    let pool = app_context::<PgPool>(cx);
    let now = now_seconds();
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    let current: Option<(i64, i64, String, bool)> = sqlx::query_as(
        "SELECT revision, size, COALESCE(object_key, ''), deleted
         FROM sync_entries WHERE user_id = $1 AND entry_id = $2 FOR UPDATE",
    )
    .bind(user.id)
    .bind(&input.entry_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (current_revision, old_size, old_key, already_deleted) =
        current.unwrap_or((0, 0, String::new(), false));

    if current_revision != input.base_revision {
        return Err(bad_request("entry revision conflict").into());
    }

    let usage: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN deleted THEN 0 ELSE size END), 0)::BIGINT
         FROM sync_entries WHERE user_id = $1",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    let new_usage = if already_deleted {
        usage
    } else {
        usage - old_size
    };
    let revision = current_revision + 1;

    sqlx::query(
        "INSERT INTO sync_entries
             (user_id, entry_id, revision, content_hash, size, ciphertext, object_key, updated_at, deleted)
         VALUES ($1, $2, $3, '', 0, NULL, NULL, $4, TRUE)
         ON CONFLICT (user_id, entry_id) DO UPDATE SET
             revision = EXCLUDED.revision,
             content_hash = '',
             size = 0,
             ciphertext = NULL,
             object_key = NULL,
             updated_at = EXCLUDED.updated_at,
             deleted = TRUE",
    )
    .bind(user.id)
    .bind(&input.entry_id)
    .bind(revision)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    if !old_key.is_empty() {
        if let Err(error) = storage.delete(&old_key).await {
            eprintln!("[sync] could not delete removed entry object: {error}");
        }
    }

    Ok(Json(SyncWriteResponse {
        entry_id: input.entry_id,
        revision,
        hash: String::new(),
        size: 0,
        usage_bytes: new_usage.max(0),
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

fn require_storage(cx: &Cx) -> Result<&SyncStorage> {
    let storage = app_context::<SyncStorage>(cx);

    if !storage.enabled() {
        return Err(service_unavailable(60).into());
    }

    Ok(storage)
}

fn validate_entry_id(entry_id: &str) -> Result<()> {
    if parse_date(entry_id).is_none() {
        return Err(bad_request("entry id must be YYYY-MM-DD").into());
    }

    Ok(())
}

fn validate_hash(hash: &str) -> Result<()> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad_request("invalid content hash").into());
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
    validate_hash(&hash)?;

    Ok(hash.to_lowercase())
}

fn valid_encoded(value: &str, minimum_bytes: usize) -> bool {
    STANDARD
        .decode(value)
        .map(|bytes| bytes.len() >= minimum_bytes)
        .unwrap_or(false)
}

async fn cleanup_expired_uploads(
    pool: &PgPool,
    storage: &SyncStorage,
    user_id: i64,
    cutoff: i64,
) -> Result<()> {
    let expired: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, object_key FROM sync_uploads
         WHERE user_id = $1 AND committed_at IS NULL AND created_at < $2
         LIMIT 100",
    )
    .bind(user_id)
    .bind(cutoff)
    .fetch_all(pool)
    .await?;

    for (id, object_key) in &expired {
        if let Err(error) = storage.delete(object_key).await {
            eprintln!("[sync] could not delete expired upload object: {error}");
        }

        sqlx::query("DELETE FROM sync_uploads WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    }

    sqlx::query(
        "DELETE FROM sync_uploads
         WHERE user_id = $1 AND committed_at IS NOT NULL AND committed_at < $2",
    )
    .bind(user_id)
    .bind(cutoff)
    .execute(pool)
    .await?;

    Ok(())
}

async fn rate_limit(cx: &Cx, user_id: i64, action: &str, max: i32) -> Result<()> {
    let pool = app_context::<PgPool>(cx);
    if !auth::rate_limit_allows(
        pool,
        &format!("sync:{action}:user:{user_id}"),
        max,
        60,
    )
    .await?
    {
        return Err(too_many_requests(60).into());
    }

    Ok(())
}

pub async fn purge_user_objects(
    pool: &PgPool,
    storage: &SyncStorage,
    user_id: i64,
) {
    if !storage.enabled() {
        return;
    }

    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT object_key FROM sync_entries WHERE user_id = $1 AND object_key IS NOT NULL
         UNION
         SELECT object_key FROM sync_manifests WHERE user_id = $1 AND object_key IS NOT NULL
         UNION
         SELECT object_key FROM sync_uploads WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for key in keys {
        if let Err(error) = storage.delete(&key).await {
            eprintln!("[sync] could not delete account object: {error}");
        }
    }
}

fn storage_error(message: String) -> topcoat::Error {
    eprintln!("[sync] object storage error: {message}");
    internal_server_error(std::io::Error::other("sync storage is unavailable")).into()
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
