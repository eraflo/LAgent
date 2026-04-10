## Description

<!-- What does this PR do? Why? Link to the related issue if applicable. -->

Closes #

## Type of Change

> The PR **title** must follow [Conventional Commits](https://www.conventionalcommits.org/):
> `feat: ...` | `fix: ...` | `docs: ...` | `refactor: ...` | `test: ...` | `ci: ...` | `chore: ...`
> Append `!` for breaking changes: `feat!: ...`

- [ ] `feat` — new language feature or capability
- [ ] `fix` — bug fix
- [ ] `perf` — performance improvement
- [ ] `refactor` — code restructuring, no behaviour change
- [ ] `docs` — documentation only
- [ ] `test` — tests only
- [ ] `ci` — CI/CD pipeline change
- [ ] `chore` — maintenance (deps, build config, …)

## Quality Gates

- [ ] `cargo fmt --all` — no formatting diff
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero warnings
- [ ] `cargo test --workspace --all-features` — all tests pass
- [ ] `cargo doc --workspace --no-deps` — docs build without errors

## New Language Features (if applicable)

If this PR adds or modifies a language feature, confirm all 7 steps are done:

- [ ] Token in `lagent-compiler/src/lexer/mod.rs`
- [ ] AST node in `lagent-compiler/src/parser/ast.rs`
- [ ] Parser rule in `lagent-compiler/src/parser/mod.rs`
- [ ] Semantic validation in `lagent-compiler/src/semantic/mod.rs`
- [ ] `OpCode` in `lagent-compiler/src/codegen/opcodes.rs` + emission in `codegen/mod.rs`
- [ ] VM dispatch in `lagent-vm/src/vm.rs`
- [ ] Unit tests + integration test

## Notes for Reviewers

<!-- Anything that makes review easier: tricky parts, design decisions, alternatives considered. -->
