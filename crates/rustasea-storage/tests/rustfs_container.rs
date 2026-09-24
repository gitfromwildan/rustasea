//! Docker-backed S3 integration tests against RustFS.
//!
//! These exercise the real `s3` disk, built through [`StorageManager`] exactly
//! as `config/storage.toml` is, against a `rustfs/rustfs` container on a
//! plain-HTTP endpoint (the local-development setup documented by
//! `config/storage.toml` and `.env.example`). They are `#[ignore]` because
//! they require a Docker daemon; run them explicitly:
//!
//! ```text
//! cargo test -p rustasea-storage --features aws --test rustfs_container -- --ignored --nocapture
//! ```

#![cfg(feature = "aws")]

use std::sync::Mutex;
use std::time::Duration;

use object_store::aws::{AwsAuthorizer, AwsCredential};
use object_store::client::{HttpConnector, HttpRequest, HttpRequestBody, ReqwestConnector};
use object_store::ClientOptions;
use rustasea_storage::{ManagedDisk, StorageManager};
use testcontainers::core::ContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

/// Root credentials the container is started with.
const ACCESS_KEY: &str = "rustasea-test";
/// Root secret the container is started with.
const SECRET_KEY: &str = "rustasea-test-secret";
/// Bucket created before each test (RustFS starts empty).
const BUCKET: &str = "rustasea-test";
/// Signing region (the SDK default, which RustFS accepts).
const REGION: &str = "us-east-1";
/// How long to retry `CreateBucket` while RustFS starts before giving up.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Every `AWS_*` variable the `s3` disk overlays onto its config.
const AWS_VARS: [&str; 5] = [
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_DEFAULT_REGION",
    "AWS_BUCKET",
    "AWS_ENDPOINT",
];

/// Serializes the process-global `AWS_*` environment across tests.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Start a RustFS container and create [`BUCKET`] once its S3 API is ready.
///
/// Returns the container (dropping it stops RustFS) and its `http://` endpoint.
async fn start_rustfs() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("rustfs/rustfs", "1.0.0")
        .with_exposed_port(ContainerPort::Tcp(9000))
        .with_env_var("RUSTFS_ACCESS_KEY", ACCESS_KEY)
        .with_env_var("RUSTFS_SECRET_KEY", SECRET_KEY)
        .start()
        .await
        .expect("rustfs container must start");
    let port = container
        .get_host_port_ipv4(9000u16)
        .await
        .expect("S3 port must be mapped");
    let endpoint = format!("http://127.0.0.1:{port}");
    create_bucket(&endpoint).await;
    (container, endpoint)
}

/// Create [`BUCKET`] with a SigV4-signed `CreateBucket` request, retrying
/// while RustFS starts.
///
/// `object_store` has no bucket-management API, so the request is signed with
/// its public [`AwsAuthorizer`] and sent through its HTTP client. RustFS
/// accepts connections (and answers `/health`) before its object layer is
/// ready and replies `503 Service Unavailable` meanwhile, so transport errors
/// (such as a refused connection) and 503s are retried until
/// [`READY_TIMEOUT`]; any other response is fatal.
async fn create_bucket(endpoint: &str) {
    let credential = AwsCredential {
        key_id: ACCESS_KEY.to_string(),
        secret_key: SECRET_KEY.to_string(),
        token: None,
    };
    let client = ReqwestConnector {}
        .connect(&ClientOptions::new().with_allow_http(true))
        .expect("HTTP client must build");
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    loop {
        let mut request = HttpRequest::new(HttpRequestBody::empty());
        *request.method_mut() = "PUT".parse().expect("PUT is a valid method");
        *request.uri_mut() = format!("{endpoint}/{BUCKET}")
            .parse()
            .expect("bucket URL must parse");
        AwsAuthorizer::new(&credential, "s3", REGION)
            .try_authorize(&mut request, None)
            .expect("CreateBucket request must sign");
        let last_error = match client.execute(request).await {
            Ok(response) if response.status().is_success() => return,
            Ok(response) if response.status().as_u16() == 503 => response.status().to_string(),
            Ok(response) => {
                let status = response.status();
                let body = response.into_body().bytes().await.unwrap_or_default();
                panic!(
                    "CreateBucket failed with {status}: {}",
                    String::from_utf8_lossy(&body)
                )
            }
            Err(error) => error.to_string(),
        };
        assert!(
            tokio::time::Instant::now() < deadline,
            "rustfs did not become ready at {endpoint}: {last_error}"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Run `body` with the `AWS_*` overlay variables set to `values`, restoring
/// the prior environment afterwards.
///
/// Variables not listed are set blank (which the overlay ignores), so an
/// ambient AWS configuration on the developer machine cannot leak in.
fn with_aws_env<T>(values: &[(&str, &str)], body: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let prior: Vec<(&str, Option<String>)> = AWS_VARS
        .iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect();
    for key in AWS_VARS {
        let value = values
            .iter()
            .find(|(name, _)| *name == key)
            .map_or("", |(_, value)| *value);
        std::env::set_var(key, value);
    }
    let result = body();
    for (key, value) in prior {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    result
}

/// Put, read back, probe, and delete one object through `disk`.
async fn assert_round_trip(disk: &dyn ManagedDisk) {
    let key = "reports/2026/hello.txt";
    disk.put(key, b"hello rustfs")
        .await
        .expect("put must succeed");
    assert!(disk.exists(key).await.expect("exists must succeed"));
    assert_eq!(
        disk.get(key).await.expect("get must succeed"),
        b"hello rustfs"
    );
    disk.delete(key).await.expect("delete must succeed");
    assert!(!disk.exists(key).await.expect("exists must succeed"));
}

#[tokio::test]
#[ignore = "requires docker"]
async fn s3_disk_round_trips_over_plain_http_endpoint() {
    let (_container, endpoint) = start_rustfs().await;
    let toml = format!(
        r#"
[storage]
default = "s3"

[storage.disks.s3]
driver = "s3"
bucket = "{BUCKET}"
region = "{REGION}"
access_key_id = "{ACCESS_KEY}"
secret_access_key = "{SECRET_KEY}"
endpoint = "{endpoint}"
"#
    );
    let manager =
        with_aws_env(&[], || StorageManager::from_toml(&toml)).expect("s3 disk must build");
    let disk = manager.disk("s3").expect("s3 disk must be registered");
    assert_round_trip(disk.as_ref()).await;
}

#[tokio::test]
#[ignore = "requires docker"]
async fn s3_disk_reads_documented_aws_environment() {
    let (_container, endpoint) = start_rustfs().await;
    // `.env.example` documents that the credentials, region, bucket, and
    // endpoint come from `AWS_*`, so the file names only the driver.
    let toml = r#"
[storage]
default = "s3"

[storage.disks.s3]
driver = "s3"
"#;
    let manager = with_aws_env(
        &[
            ("AWS_ACCESS_KEY_ID", ACCESS_KEY),
            ("AWS_SECRET_ACCESS_KEY", SECRET_KEY),
            ("AWS_DEFAULT_REGION", REGION),
            ("AWS_BUCKET", BUCKET),
            ("AWS_ENDPOINT", &endpoint),
        ],
        || StorageManager::from_toml(toml),
    )
    .expect("s3 disk must build");
    let disk = manager.disk("s3").expect("s3 disk must be registered");
    assert_round_trip(disk.as_ref()).await;
}
