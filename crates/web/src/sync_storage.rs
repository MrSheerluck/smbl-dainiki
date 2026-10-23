use std::{env, time::Duration};

use aws_sdk_s3::{
    Client,
    config::{BehaviorVersion, Credentials, Region},
    presigning::PresigningConfig,
    primitives::ByteStream,
};

#[derive(Clone)]
pub struct SyncStorage {
    inner: Option<S3>,
}

#[derive(Clone)]
struct S3 {
    client: Client,
    bucket: String,
}

impl SyncStorage {
    pub fn from_env() -> Self {
        if env::var("SYNC_STORAGE").as_deref() == Ok("disabled") {
            return Self { inner: None };
        }

        let endpoint = env::var("R2_ENDPOINT").ok();
        let access_key = env::var("R2_ACCESS_KEY_ID").ok();
        let secret_key = env::var("R2_SECRET_ACCESS_KEY").ok();
        let bucket = env::var("R2_BUCKET").ok();

        let (Some(endpoint), Some(access_key), Some(secret_key), Some(bucket)) =
            (endpoint, access_key, secret_key, bucket)
        else {
            eprintln!("[sync] object storage is not configured; encrypted sync is disabled");
            return Self { inner: None };
        };

        let region = env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string());
        let force_path_style = env::var("R2_FORCE_PATH_STYLE")
            .map(|value| value != "false" && value != "0")
            .unwrap_or(true);
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "dainiki-sync",
            ))
            .region(Region::new(region))
            .endpoint_url(&endpoint)
            .force_path_style(force_path_style)
            .build();

        eprintln!("[sync] object storage enabled for bucket {bucket}");

        Self {
            inner: Some(S3 {
                client: Client::from_conf(config),
                bucket,
            }),
        }
    }

    pub fn enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub async fn presign_put(&self, key: &str) -> Result<String, String> {
        let inner = self.s3()?;
        let expires = PresigningConfig::expires_in(Duration::from_secs(15 * 60))
            .map_err(|error| error.to_string())?;
        let request = inner
            .client
            .put_object()
            .bucket(&inner.bucket)
            .key(key)
            .presigned(expires)
            .await
            .map_err(|error| error.to_string())?;

        Ok(request.uri().to_string())
    }

    pub async fn presign_get(&self, key: &str) -> Result<String, String> {
        let inner = self.s3()?;
        let expires = PresigningConfig::expires_in(Duration::from_secs(15 * 60))
            .map_err(|error| error.to_string())?;
        let request = inner
            .client
            .get_object()
            .bucket(&inner.bucket)
            .key(key)
            .presigned(expires)
            .await
            .map_err(|error| error.to_string())?;

        Ok(request.uri().to_string())
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), String> {
        let inner = self.s3()?;
        inner
            .client
            .put_object()
            .bucket(&inner.bucket)
            .key(key)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn size(&self, key: &str) -> Result<i64, String> {
        let inner = self.s3()?;
        let output = inner
            .client
            .head_object()
            .bucket(&inner.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| error.to_string())?;

        Ok(output.content_length().unwrap_or_default())
    }

    pub async fn delete(&self, key: &str) -> Result<(), String> {
        let inner = self.s3()?;
        inner
            .client
            .delete_object()
            .bucket(&inner.bucket)
            .key(key)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn s3(&self) -> Result<&S3, String> {
        self.inner
            .as_ref()
            .ok_or_else(|| "sync object storage is not configured".to_string())
    }
}

