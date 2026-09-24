//! Hermetic unit tests for the S3 disk configuration (no network).

use super::S3DiskConfig;
use crate::test_support::with_env;

/// A file-sourced config for the environment overlay to act on.
fn file_config() -> S3DiskConfig {
    S3DiskConfig {
        bucket: "file-bucket".into(),
        region: Some("ap-southeast-1".into()),
        access_key_id: Some("file-key".into()),
        secret_access_key: Some("file-secret".into()),
        endpoint: Some("https://file.example.com".into()),
        settings: Default::default(),
    }
}

#[test]
fn apply_env_overlays_documented_variables() {
    let mut config = file_config();
    with_env(
        &[
            ("AWS_ACCESS_KEY_ID", "env-key"),
            ("AWS_SECRET_ACCESS_KEY", "env-secret"),
            ("AWS_DEFAULT_REGION", "eu-west-1"),
            ("AWS_BUCKET", "env-bucket"),
            ("AWS_ENDPOINT", "http://env.example.com:9000"),
        ],
        || config.apply_env(),
    );
    assert_eq!(config.access_key_id.as_deref(), Some("env-key"));
    assert_eq!(config.secret_access_key.as_deref(), Some("env-secret"));
    assert_eq!(config.region.as_deref(), Some("eu-west-1"));
    assert_eq!(config.bucket, "env-bucket");
    assert_eq!(
        config.endpoint.as_deref(),
        Some("http://env.example.com:9000")
    );
}

#[test]
fn apply_env_ignores_blank_values() {
    let mut config = file_config();
    with_env(
        &[
            ("AWS_ACCESS_KEY_ID", ""),
            ("AWS_SECRET_ACCESS_KEY", "   "),
            ("AWS_DEFAULT_REGION", ""),
            ("AWS_BUCKET", "  "),
            ("AWS_ENDPOINT", ""),
        ],
        || config.apply_env(),
    );
    assert_eq!(config, file_config());
}

#[test]
fn debug_output_redacts_secret_access_key() {
    let config = S3DiskConfig {
        secret_access_key: Some("super-secret-value".into()),
        ..file_config()
    };
    let rendered = format!("{config:?}");
    assert!(!rendered.contains("super-secret-value"));
    assert!(rendered.contains("[REDACTED]"));
    assert!(rendered.contains("file-bucket"));
}

#[cfg(feature = "aws")]
mod builder {
    use object_store::aws::AmazonS3ConfigKey;
    use object_store::ClientConfigKey;

    use super::file_config;
    use crate::s3::S3DiskConfig;
    use crate::test_support::with_env;

    /// The `allow_http` value the builder hands to `object_store`.
    fn allow_http(endpoint: Option<&str>) -> Option<String> {
        let config = S3DiskConfig {
            endpoint: endpoint.map(str::to_string),
            ..file_config()
        };
        config
            .builder()
            .get_config_value(&AmazonS3ConfigKey::Client(ClientConfigKey::AllowHttp))
    }

    #[test]
    fn plain_http_is_allowed_only_for_http_endpoints() {
        let cases = [
            (Some("http://127.0.0.1:9000"), "true"),
            (Some("HTTP://rustfs:9000"), "true"),
            (Some(" http://127.0.0.1:9000 "), "true"),
            (Some("https://s3.example.com"), "false"),
            (None, "false"),
        ];
        for (endpoint, want) in cases {
            assert_eq!(
                allow_http(endpoint).as_deref(),
                Some(want),
                "endpoint {endpoint:?}"
            );
        }
    }

    #[test]
    fn built_disk_applies_the_environment_overlay() {
        let toml = r#"
[storage]
default = "s3"

[storage.disks.s3]
driver = "s3"
bucket = "file-bucket"
access_key_id = "file-key"
secret_access_key = "file-secret"
"#;
        let mut label = String::new();
        with_env(&[("AWS_BUCKET", "env-bucket")], || {
            let manager = crate::StorageManager::from_toml(toml).expect("s3 disk must build");
            label = manager
                .disk("s3")
                .expect("s3 disk must be registered")
                .label();
        });
        assert_eq!(label, "s3://env-bucket");
    }

    #[test]
    fn bucket_can_come_from_aws_bucket_alone() {
        let toml = r#"
[storage]
default = "s3"

[storage.disks.s3]
driver = "s3"
"#;
        let mut label = String::new();
        with_env(&[("AWS_BUCKET", "env-bucket")], || {
            let manager = crate::StorageManager::from_toml(toml).expect("s3 disk must build");
            label = manager
                .disk("s3")
                .expect("s3 disk must be registered")
                .label();
        });
        assert_eq!(label, "s3://env-bucket");
    }

    #[test]
    fn blank_bucket_is_rejected() {
        let toml = r#"
[storage]
default = "s3"

[storage.disks.s3]
driver = "s3"
bucket = "  "
"#;
        let mut outcome = None;
        with_env(&[("AWS_BUCKET", "")], || {
            outcome = Some(crate::StorageManager::from_toml(toml).map(|_| ()));
        });
        assert!(
            matches!(outcome, Some(Err(crate::StorageError::Config(_)))),
            "got {outcome:?}"
        );
    }
}
