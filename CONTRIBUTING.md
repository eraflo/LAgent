# Contributing to L-Agent

Thank you for your interest in L-Agent!

## Development Setup

Prerequisites: Rust 1.78+ (install via [rustup](https://rustup.rs)).

```bash
git clone https://github.com/lagent-lang/lagent
cd lagent
cargo build
cargo test
```

## Project Structure

See `docs/ARCHITECTURE.md` for a detailed description of each crate.

## Workflow

1. Fork the repository and create a branch: `git checkout -b feat/my-feature`
2. Make your changes with tests.
3. Run the quality gates (see below) — toutes doivent passer.
4. Open a pull request against `main`.

## Quality Gates

Ces commandes doivent toutes passer avant d'ouvrir une PR (et sont vérifiées en CI) :

```bash
# 1. Formatage — aucune diff tolérée
cargo fmt --all

# 2. Linting — zéro warning Clippy autorisé
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Tests complets
cargo test --workspace --all-features

# 4. Documentation — aucune erreur de doc
cargo doc --workspace --no-deps
```

Raccourci recommandé (tout en une commande) :

```bash
cargo fmt --all && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace --all-features
```

## Coding Standards

- Tous les items publics (`pub`) **doivent** avoir un doc comment (`///`).
- Pas de `unwrap()` ni de `expect()` dans le code de production — utiliser `?` avec `anyhow`/`thiserror`.
- Toute nouvelle fonctionnalité du langage requiert les 7 étapes suivantes :
  1. **Token** dans `lagent-compiler/src/lexer/mod.rs`
  2. **Nœud AST** dans `lagent-compiler/src/parser/ast.rs`
  3. **Règle parser** dans `lagent-compiler/src/parser/mod.rs`
  4. **Validation sémantique** dans `lagent-compiler/src/semantic/mod.rs`
  5. **`OpCode`** dans `lagent-compiler/src/codegen/opcodes.rs` + émission dans `codegen/mod.rs`
  6. **Dispatch VM** dans `lagent-vm/src/vm.rs`
  7. **Tests** : unitaires inline + test d'intégration dans `tests/`

## Reporting Issues

Use GitHub Issues. For language design discussions, open a Discussion instead.
