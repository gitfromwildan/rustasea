# rustasea-storage

RustaSea storage: read-through disk Storage, path confinement, and local disk.

Part of the [RustaSea framework](https://github.com/rustasea/framework) - a Laravel-inspired developer experience for Rust.

## Usage

```toml
[dependencies]
rustasea-storage = "0.1"
```

## S3 and S3-compatible disks

Enable the `aws` feature (or `storage-s3` on the `rustasea` facade crate) to
build `driver = "s3"` disks from `config/storage.toml`. `AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, `AWS_DEFAULT_REGION`, `AWS_BUCKET`, and `AWS_ENDPOINT`
override the matching keys on every `s3` disk (blank values are ignored). An
`http://` endpoint enables plain HTTP for local S3-compatible services such as
RustFS or MinIO; any other endpoint stays HTTPS-only.

The `aws` feature needs rustc 1.89 or newer: `object_store`'s AWS backend
depends on `crc-fast` 1.10, which raised its MSRV above the workspace's 1.88.

The Docker-backed RustFS suite runs with:

```text
cargo test -p rustasea-storage --features aws --test rustfs_container -- --ignored
```

## License

MIT - see [LICENSE-MIT](https://github.com/rustasea/framework/blob/master/LICENSE-MIT).
