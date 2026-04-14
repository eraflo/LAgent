# Contributing to Wispee

Thank you for your interest in Wispee!

## Development Setup

Prerequisites: Rust 1.78+ (install via [rustup](https://rustup.rs)).

```bash
git clone https://github.com/eraflo/Wispee
cd Wispee
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
  1. **Token** dans `wispee-compiler/src/lexer/mod.rs`
  2. **Nœud AST** dans `wispee-compiler/src/parser/ast.rs`
  3. **Règle parser** dans `wispee-compiler/src/parser/mod.rs`
  4. **Validation sémantique** dans `wispee-compiler/src/semantic/mod.rs`
  5. **`OpCode`** dans `wispee-compiler/src/codegen/opcodes.rs` + émission dans `codegen/mod.rs`
  6. **Dispatch VM** dans `wispee-vm/src/vm.rs`
  7. **Tests** : unitaires inline + test d'intégration dans `tests/`

## Branches & Release Flow

### Branches

| Branch    | Purpose                                  |
|-----------|------------------------------------------|
| `main`    | Production — always releasable           |
| `develop` | Integration branch for in-progress work |
| `feat/*`  | Feature branches, opened against `main` |
| `fix/*`   | Bug fix branches                         |

### Commit Message Convention

All commits **must** follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

| Type       | Triggers version bump  | Example                              |
|------------|------------------------|--------------------------------------|
| `feat`     | minor (`0.x.0`)        | `feat: add soul keyword`             |
| `fix`      | patch (`0.0.x`)        | `fix: ctx_free double-free crash`    |
| `feat!` / `BREAKING CHANGE` | major (`x.0.0`) | `feat!: rename ctx_alloc to ctx_new` |
| `perf`     | patch                  | `perf: reduce token heap alloc`      |
| `docs`     | no bump                | `docs: update SPEC.md §8`            |
| `refactor` | no bump                | `refactor: split lexer into modules` |
| `test`     | no bump                | `test: add kernel verify unit tests` |
| `ci`       | no bump                | `ci: pin rust to 1.78`               |
| `chore`    | no bump                | `chore(deps): bump logos to 0.15`    |

### Automated Release Process (release-plz)

Releases are **fully automated** via [release-plz](https://release-plz.dev):

```
feat/fix commit merged to main
        ↓
release-plz analyses Conventional Commits
        ↓
Opens a "Release PR":  chore(release): v0.2.0
  • Bumps version in Cargo.toml (workspace)
  • Generates / updates CHANGELOG.md
        ↓
Merge the Release PR
        ↓
release-plz creates a git tag + GitHub Release
  • Attaches cross-platform binaries (built by build-release.yml)
  • (opt.) Publishes crates to crates.io
```

**You never manually bump versions or write changelog entries.**

## Reporting Issues

Use GitHub Issues (use the provided templates). For language design discussions, open a **Discussion** instead.
