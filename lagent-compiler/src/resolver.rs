// SPDX-License-Identifier: Apache-2.0
//! Module resolver: expands `use "path.la"` declarations by inline-including
//! the parsed items from the referenced file.
//!
//! Resolution is performed as a pre-processing step before semantic analysis.
//! Circular imports are not detected in Phase 4.

use crate::lexer::tokenize;
use crate::parser::{ast::Item, parse};
use anyhow::{Context, Result};
use std::path::Path;

/// Walk `items`, replacing every [`Item::UseDecl`] with the items parsed from
/// the referenced file (relative to `base_dir`).
///
/// Non-`pub` items from the imported module are included in the flat list
/// (no enforcement of visibility yet — Phase 5).
///
/// # Errors
///
/// Returns an error if a referenced file cannot be read or parsed.
pub fn resolve_uses(items: Vec<Item>, base_dir: &Path) -> Result<Vec<Item>> {
    let mut out: Vec<Item> = Vec::new();
    for item in items {
        if let Item::UseDecl(u) = item {
            let full = base_dir.join(&u.path);
            let src = std::fs::read_to_string(&full)
                .with_context(|| format!("cannot open module `{}`", u.path))?;
            let tokens =
                tokenize(&src).with_context(|| format!("lex error in module `{}`", u.path))?;
            let imported =
                parse(tokens).with_context(|| format!("parse error in module `{}`", u.path))?;
            // Recursively resolve nested `use` declarations.
            let resolved = resolve_uses(imported, base_dir)?;
            out.extend(resolved);
        } else {
            out.push(item);
        }
    }
    Ok(out)
}
