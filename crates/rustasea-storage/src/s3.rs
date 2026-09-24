//! S3 disk configuration (`[storage.disks.*]` with `driver = "s3"`).
//!
//! The type is compiled unconditionally (the facade must parse an `s3` disk
//! even when the `aws` feature is off); only the facade's `object_store`
//! builder is feature-gated.

use serde::Deserialize;

use crate::facade::DiskSettings;

/// Configuration for an S3 disk.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct S3DiskConfig {
    /// Bucket name.
    pub bucket: String,
    /// AWS region (falls back to the SDK default when absent).
    #[serde(default)]
    pub region: Option<String>,
    /// Explicit access key id (prefer environment/secret manager).
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Explicit secret access key (prefer environment/secret manager).
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Custom endpoint for S3-compatible services (MinIO, R2).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Shared Laravel-parity disk settings.
    #[serde(flatten, default)]
    pub settings: DiskSettings,
}
