//! S3 disk configuration (`[storage.disks.*]` with `driver = "s3"`).
//!
//! Mirrors Laravel's `s3` disk keys (`bucket`, `region`, `key`/`secret` as
//! `access_key_id`/`secret_access_key`, `endpoint`). The type is compiled
//! unconditionally (the facade must parse an `s3` disk even when the `aws`
//! feature is off); only the `object_store` builder is feature-gated.
//!
//! # Environment bridge
//!
//! [`S3DiskConfig::apply_env`] overlays the Laravel-style `AWS_*` variables
//! that `.env.example` documents: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
//! `AWS_DEFAULT_REGION`, `AWS_BUCKET`, and `AWS_ENDPOINT`.
//!
//! # Plain-HTTP endpoints
//!
//! `object_store` rejects non-TLS URLs unless `allow_http` is set. An explicit
//! `http://` endpoint (RustFS, MinIO, or another S3-compatible service in
//! local development) opts in to plain HTTP; any other endpoint, and AWS
//! itself, stays HTTPS-only.

use serde::Deserialize;

use crate::error::{Result, StorageError};
use crate::facade::{env_non_empty, DiskSettings};

/// Typed `[storage.disks.*]` configuration for the `s3` driver.
///
/// `secret_access_key` is redacted from [`Debug`] output so a debug dump or
/// log line can never leak the credential.
#[derive(Clone, Deserialize, PartialEq, Eq)]
pub struct S3DiskConfig {
    /// Bucket name (may be left out when `AWS_BUCKET` supplies it).
    #[serde(default)]
    pub bucket: String,
    /// AWS region (falls back to the SDK default when absent).
    #[serde(default)]
    pub region: Option<String>,
    /// Explicit access key id (prefer `AWS_ACCESS_KEY_ID` or a secret manager).
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Explicit secret access key (prefer `AWS_SECRET_ACCESS_KEY` or a secret
    /// manager).
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Custom endpoint for S3-compatible services (RustFS, MinIO, R2).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Shared Laravel-parity disk settings.
    #[serde(flatten, default)]
    pub settings: DiskSettings,
}

impl std::fmt::Debug for S3DiskConfig {
    /// Render the config with the `secret_access_key` masked.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3DiskConfig")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key_id", &self.access_key_id)
            .field(
                "secret_access_key",
                &self.secret_access_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field("endpoint", &self.endpoint)
            .field("settings", &self.settings)
            .finish()
    }
}

impl S3DiskConfig {
    /// Overlay the documented `AWS_*` environment variables.
    ///
    /// The Laravel `s3` disk names (`AWS_ACCESS_KEY_ID`,
    /// `AWS_SECRET_ACCESS_KEY`, `AWS_DEFAULT_REGION`, `AWS_BUCKET`,
    /// `AWS_ENDPOINT`) override the matching file keys; a blank value is
    /// ignored. Like the `SFTP_*` overlay for `sftp` disks, it applies to every
    /// `s3` disk.
    pub fn apply_env(&mut self) {
        if let Some(value) = env_non_empty("AWS_ACCESS_KEY_ID") {
            self.access_key_id = Some(value);
        }
        if let Some(value) = env_non_empty("AWS_SECRET_ACCESS_KEY") {
            self.secret_access_key = Some(value);
        }
        if let Some(value) = env_non_empty("AWS_DEFAULT_REGION") {
            self.region = Some(value);
        }
        if let Some(value) = env_non_empty("AWS_BUCKET") {
            self.bucket = value;
        }
        if let Some(value) = env_non_empty("AWS_ENDPOINT") {
            self.endpoint = Some(value);
        }
    }

    /// Validate that the mandatory keys are present (after the env overlay).
    ///
    /// # Errors
    ///
    /// [`StorageError::Config`] when `bucket` is blank, i.e. neither the file
    /// nor `AWS_BUCKET` supplied one.
    pub fn validate(&self) -> Result<()> {
        if self.bucket.trim().is_empty() {
            return Err(StorageError::Config(
                "s3 disk requires a non-empty `bucket` (or `AWS_BUCKET`)".into(),
            ));
        }
        Ok(())
    }

    /// Translate the config into an `object_store` S3 builder.
    ///
    /// An `http://` endpoint enables `allow_http` (see the module docs).
    #[cfg(feature = "aws")]
    pub(crate) fn builder(&self) -> object_store::aws::AmazonS3Builder {
        let mut builder =
            object_store::aws::AmazonS3Builder::new().with_bucket_name(self.bucket.clone());
        if let Some(region) = &self.region {
            builder = builder.with_region(region.clone());
        }
        if let Some(key_id) = &self.access_key_id {
            builder = builder.with_access_key_id(key_id.clone());
        }
        if let Some(secret) = &self.secret_access_key {
            builder = builder.with_secret_access_key(secret.clone());
        }
        if let Some(endpoint) = &self.endpoint {
            // URL parsing ignores surrounding whitespace, so the scheme check must too.
            let endpoint = endpoint.trim();
            builder = builder
                .with_endpoint(endpoint)
                .with_allow_http(is_plain_http(endpoint));
        }
        builder
    }
}

/// Whether `endpoint` uses the plain-HTTP scheme (case-insensitive).
#[cfg(feature = "aws")]
fn is_plain_http(endpoint: &str) -> bool {
    endpoint
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
}

#[cfg(test)]
mod tests;
