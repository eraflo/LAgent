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
3. Run `cargo test --workspace` and `cargo clippy --workspace`.
4. Open a pull request against `main`.

## Coding Standards

- Format with `cargo fmt --all` before committing.
- All public items must have doc comments.
- New language features require: lexer token + parser rule + semantic check + codegen + VM dispatch + test.

## Reporting Issues

Use GitHub Issues. For language design discussions, open a Discussion instead.
