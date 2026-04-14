// SPDX-License-Identifier: Apache-2.0
//! `wispee.toml` project manifest parsing.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Top-level `wispee.toml` configuration.
#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    /// `[project]` section — always required.
    pub project: ProjectMeta,
    /// `[lib]` section — present when the project is a library.
    pub lib: Option<LibConfig>,
}

/// `[project]` section of `wispee.toml`.
#[derive(Debug, Deserialize)]
pub struct ProjectMeta {
    /// Project name (used as the library bundle name when `--lib` is set).
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Default entry-point source file (relative to the project root).
    pub entry: String,
}

/// `[lib]` section of `wispee.toml` — declares a library crate.
#[derive(Debug, Deserialize)]
pub struct LibConfig {
    /// Library entry-point source file (relative to the project root).
    pub entry: String,
    /// Library bundle name (used as the `.walb` filename stem).
    pub name: String,
}

impl ProjectConfig {
    /// Load and parse a `wispee.toml` file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text)?;
        Ok(config)
    }

    /// Walk up from `start_dir` looking for a `wispee.toml`.
    ///
    /// Returns `(config, project_root)` on success, or `None` if not found.
    #[must_use]
    pub fn find(start_dir: &Path) -> Option<(Self, PathBuf)> {
        let mut dir = start_dir.to_path_buf();
        loop {
            let candidate = dir.join("wispee.toml");
            if candidate.exists() {
                if let Ok(cfg) = Self::load(&candidate) {
                    return Some((cfg, dir));
                }
            }
            if !dir.pop() {
                return None;
            }
        }
    }
}
