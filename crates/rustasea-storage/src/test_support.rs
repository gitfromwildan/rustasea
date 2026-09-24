//! Test-only helpers shared by the crate's unit tests.

use std::sync::Mutex;

/// Serializes environment-mutating tests (precedent: broadcast `manager/tests.rs`).
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Run `body` with the given env vars set, restoring prior values afterwards.
pub(crate) fn with_env<F: FnOnce()>(vars: &[(&str, &str)], body: F) {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let prior: Vec<(String, Option<String>)> = vars
        .iter()
        .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
        .collect();
    for (key, value) in vars {
        std::env::set_var(key, value);
    }
    body();
    for (key, value) in prior {
        match value {
            Some(value) => std::env::set_var(&key, value),
            None => std::env::remove_var(&key),
        }
    }
}
