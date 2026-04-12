// SPDX-License-Identifier: Apache-2.0
//! Persistent key-value store trait and file-backed implementation for cross-run memory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A key-value store that persists data across program runs.
///
/// Implementations of this trait are attached to the VM via
/// [`Vm::with_persistent_store`](crate::vm::Vm::with_persistent_store).
/// Operations are silently ignored when no store is configured.
pub trait PersistentStore: Send + Sync {
    /// Return the value associated with `key`, or `None` if absent.
    fn load(&self, key: &str) -> Option<String>;
    /// Store `value` under `key`, persisting immediately.
    fn save(&mut self, key: &str, value: &str);
    /// Delete `key` from the store.
    fn delete(&mut self, key: &str);
}

/// A file-backed persistent store serialised as JSON.
///
/// All mutations are flushed atomically (write-then-rename) to avoid
/// partial writes on crash.
pub struct FilePersistentStore {
    path: PathBuf,
    cache: HashMap<String, String>,
}

impl FilePersistentStore {
    /// Open (or create) a persistent store at `path`.
    ///
    /// If the file exists it is read and deserialised. If it does not exist
    /// an empty store is returned; the file is created on the first write.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn open(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        let cache = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            serde_json::from_str(&text)?
        } else {
            HashMap::new()
        };
        Ok(Self { path, cache })
    }

    fn flush(&self) -> anyhow::Result<()> {
        // Write atomically: serialise to a temp file then rename.
        let tmp = self.path.with_extension("tmp");
        let text = serde_json::to_string_pretty(&self.cache)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl PersistentStore for FilePersistentStore {
    fn load(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn save(&mut self, key: &str, value: &str) {
        self.cache.insert(key.to_string(), value.to_string());
        // Best-effort flush — ignore errors at runtime.
        let _ = self.flush();
    }

    fn delete(&mut self, key: &str) {
        self.cache.remove(key);
        let _ = self.flush();
    }
}

/// A simple in-memory persistent store for testing.
#[derive(Default)]
pub struct InMemoryPersistentStore {
    data: HashMap<String, String>,
}

impl InMemoryPersistentStore {
    /// Create a new empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return an immutable reference to the underlying map (for test assertions).
    #[must_use]
    pub fn data(&self) -> &HashMap<String, String> {
        &self.data
    }
}

impl PersistentStore for InMemoryPersistentStore {
    fn load(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    fn save(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    fn delete(&mut self, key: &str) {
        self.data.remove(key);
    }
}

// ─── File path helper ─────────────────────────────────────────────────────────

/// Walk up the directory tree from `start` looking for `lagent.toml`;
/// returns the directory containing it, or `None` if not found.
/// (Used by CLI to locate the project root for a default store path.)
#[must_use]
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("lagent.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
