// SPDX-License-Identifier: Apache-2.0
//! Module resolver: expands `use "path.la"` declarations by inline-including
//! the parsed items from the referenced file.
//!
//! Resolution is performed as a pre-processing step before semantic analysis.
//! Circular imports are not detected (planned for a future phase).

use crate::lexer::tokenize;
use crate::parser::ast::{
    ConstraintDef, FnDef, Item, KernelDef, LoreDecl, OracleDecl, SkillDef, SpellDef, TypeAlias,
};
use crate::parser::parse;
use anyhow::{Context, Result};
use std::path::Path;

/// Walk `items`, replacing every [`Item::UseDecl`] with the items parsed from
/// the referenced file (relative to `base_dir`).
///
/// Only `pub` items from imported modules are made visible to the importing
/// file (Phase 5 visibility enforcement).
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
            // Only export `pub` items across module boundaries.
            let public: Vec<Item> = resolved.into_iter().filter(item_is_pub).collect();
            out.extend(public);
        } else {
            out.push(item);
        }
    }
    Ok(out)
}

/// Return `true` if `item` is marked `pub` (or is always-visible, e.g. `SoulDef`).
///
/// Items without a visibility field (e.g. `SoulDef`, `MemoryDecl`, `UseDecl`)
/// are not re-exported from modules.
fn item_is_pub(item: &Item) -> bool {
    match item {
        Item::FnDef(FnDef { is_pub, .. })
        | Item::KernelDef(KernelDef { is_pub, .. })
        | Item::SkillDef(SkillDef { is_pub, .. })
        | Item::SpellDef(SpellDef { is_pub, .. })
        | Item::TypeAlias(TypeAlias { is_pub, .. })
        | Item::OracleDecl(OracleDecl { is_pub, .. })
        | Item::ConstraintDef(ConstraintDef { is_pub, .. })
        | Item::LoreDecl(LoreDecl { is_pub, .. }) => *is_pub,
        // SoulDef, MemoryDecl, UseDecl are not re-exported from modules.
        Item::SoulDef(_) | Item::MemoryDecl(_) | Item::UseDecl(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize as lex;
    use crate::parser::parse;

    #[test]
    fn private_fn_not_exported() {
        // Private function: item_is_pub should return false.
        let tokens = lex("fn private() {}").unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        assert!(!item_is_pub(&items[0]));
    }

    #[test]
    fn pub_fn_is_exported() {
        let tokens = lex("pub fn exported() {}").unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        assert!(item_is_pub(&items[0]));
    }

    #[test]
    fn pub_skill_is_exported() {
        let tokens = lex("pub skill Greet(n: str) -> str { return n; }").unwrap();
        let items = parse(tokens).unwrap();
        assert_eq!(items.len(), 1);
        assert!(item_is_pub(&items[0]));
    }
}
