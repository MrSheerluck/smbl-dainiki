use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct SyncStatus {
    pub active: bool,
    pub linked: bool,
    pub usage_bytes: i64,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncKeys {
    pub salt: String,
    pub wrapped_key: String,
    pub recovery_wrapped_key: String,
}

#[derive(Debug, Serialize)]
struct NewSyncKeys<'a> {
    salt: &'a str,
    wrapped_key: &'a str,
    recovery_wrapped_key: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct EntryWrite {
    pub revision: i64,
    pub hash: String,
    pub size: i64,
}

#[derive(Debug)]
pub struct RemoteManifest {
    pub revision: i64,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct RemoteEntry {
    pub revision: i64,
    pub hash: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum SyncError {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Quota,
    Message(String),
}

impl SyncError {
    pub fn message(&self) -> String {
        match self {
            Self::Unauthorized => "Sign in to use sync.".to_string(),
            Self::Forbidden => "An active sync subscription is required.".to_string(),
            Self::NotFound => "Sync data was not found.".to_string(),
            Self::Conflict => "This entry changed on another device.".to_string(),
            Self::Quota => "The sync storage quota is full.".to_string(),
            Self::Message(message) => message.clone(),
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

pub async fn fetch_status() -> Result<SyncStatus, SyncError> {
    let response = Request::get("/api/sync/status")
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => response.json().await.map_err(network_error),
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        _ => Err(response_error(response).await),
    }
}

pub async fn fetch_keys() -> Result<SyncKeys, SyncError> {
    let response = Request::get("/api/sync/keys")
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => response.json().await.map_err(network_error),
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        404 => Err(SyncError::NotFound),
        _ => Err(response_error(response).await),
    }
}

pub async fn create_keys(
    salt: &str,
    wrapped_key: &str,
    recovery_wrapped_key: &str,
) -> Result<SyncKeys, SyncError> {
    let body = NewSyncKeys {
        salt,
        wrapped_key,
        recovery_wrapped_key,
    };
    let response = Request::post("/api/sync/keys")
        .json(&body)
        .map_err(network_error)?
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => response.json().await.map_err(network_error),
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        _ => Err(response_error(response).await),
    }
}

pub async fn fetch_manifest() -> Result<Option<RemoteManifest>, SyncError> {
    let response = Request::get("/api/sync/manifest")
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => {
            let headers = response.headers();
            let revision = header_i64(&headers, "x-sync-revision")?;
            let bytes = response.binary().await.map_err(network_error)?;

            Ok(Some(RemoteManifest { revision, bytes }))
        }
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        404 => Ok(None),
        _ => Err(response_error(response).await),
    }
}

pub async fn put_manifest(
    base_revision: i64,
    bytes: Vec<u8>,
    hash: &str,
) -> Result<(), SyncError> {
    let response = Request::put(&format!("/api/sync/manifest/{base_revision}"))
        .header("content-type", "application/octet-stream")
        .header("x-sync-hash", hash)
        .body(js_sys::Uint8Array::from(bytes.as_slice()))
        .map_err(network_error)?
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => Ok(()),
        400 => Err(SyncError::Conflict),
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        413 => Err(SyncError::Quota),
        _ => Err(response_error(response).await),
    }
}

pub async fn get_entry(entry_id: &str) -> Result<Option<RemoteEntry>, SyncError> {
    let response = Request::get(&format!("/api/sync/entry/{entry_id}"))
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => {
            let headers = response.headers();
            let revision = header_i64(&headers, "x-sync-revision")?;
            let hash = headers
                .get("x-sync-hash")
                .ok_or_else(|| SyncError::Message("Sync response is missing a hash.".to_string()))?;
            let bytes = response.binary().await.map_err(network_error)?;

            Ok(Some(RemoteEntry {
                revision,
                hash,
                bytes,
            }))
        }
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        404 => Ok(None),
        _ => Err(response_error(response).await),
    }
}

pub async fn put_entry(
    entry_id: &str,
    base_revision: i64,
    bytes: Vec<u8>,
    hash: &str,
) -> Result<EntryWrite, SyncError> {
    let response = Request::put(&format!("/api/sync/entry/{entry_id}/{base_revision}"))
        .header("content-type", "application/octet-stream")
        .header("x-sync-hash", hash)
        .body(js_sys::Uint8Array::from(bytes.as_slice()))
        .map_err(network_error)?
        .send()
        .await
        .map_err(network_error)?;

    match response.status() {
        200 => response.json().await.map_err(network_error),
        400 => Err(SyncError::Conflict),
        401 => Err(SyncError::Unauthorized),
        403 => Err(SyncError::Forbidden),
        413 => Err(SyncError::Quota),
        _ => Err(response_error(response).await),
    }
}

fn header_i64(headers: &gloo_net::http::Headers, name: &str) -> Result<i64, SyncError> {
    headers
        .get(name)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| SyncError::Message(format!("Sync response is missing {name}.")))
}

async fn response_error(response: gloo_net::http::Response) -> SyncError {
    let status = response.status();
    let detail = response.text().await.unwrap_or_default();

    if detail.trim().is_empty() {
        SyncError::Message(format!("Sync request failed ({status})."))
    } else {
        SyncError::Message(format!("Sync request failed ({status}): {detail}"))
    }
}

fn network_error(error: gloo_net::Error) -> SyncError {
    SyncError::Message(format!("Could not reach sync: {error}"))
}
