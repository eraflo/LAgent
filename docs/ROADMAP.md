# L-Agent Development Roadmap

## Phase 1 — Proof of Concept (Months 1–3)
Target: a working pipeline from source to execution for a minimal subset.

### Compiler
- [ ] Complete `chumsky`-based parser for: `fn`, `let`, `ctx_alloc`, `ctx_free`, `ctx_append`, `println`
- [ ] Semantic analysis: basic name resolution and primitive type checking
- [ ] Bytecode emission for the above subset

### VM
- [ ] OpCode dispatcher loop
- [ ] `CtxAlloc`, `CtxFree`, `CtxAppend` opcodes
- [ ] `Println` opcode
- [ ] `Halt` opcode
- [ ] First local inference call via `candle`

### Tooling
- [ ] `lagent build` and `lagent run` commands working end-to-end
- [ ] `lagent check` for syntax errors
- [ ] Basic test suite (compile + run `examples/hello.la`)

---

## Phase 2 — Minimal Viable Language (Months 4–6)
Target: support for probabilistic branching, semantic types, and remote inference.

### Language
- [ ] `branch` / `case` / `default` syntax and semantics
- [ ] `type Name = semantic(...)` declaration
- [ ] Semantic type validation (embedding distance)
- [ ] Agent vocabulary keywords : `soul`, `skill`, `instruction`, `spell`, `memory`, `oracle`, `constraint`, `lore`

### Module System (linking & libraries)
- [ ] Mot-clé `use` pour importer depuis un autre fichier `.la` : `use "path/to/module.la";`
- [ ] Modificateur `pub` sur `fn`, `kernel`, `type`, `soul`, `skill` pour contrôler la visibilité
- [ ] Résolution des chemins à la compilation (relatifs au `lagent.toml` ou au fichier source)
- [ ] Déclaration de bibliothèque : section `[lib]` dans `lagent.toml` + entrée `lib = "src/lib.la"`
- [ ] Compilation d'une lib vers un `.lalb` (L-Agent Library Bundle) : bytecode + table des exports
- [ ] `lagent add <lib>` : installation depuis un registre (Phase 4)

### VM
- [ ] `Branch` opcode with constrained decoding
- [ ] Remote backend (Anthropic/OpenAI via `reqwest`)
- [ ] Feature flags for backend selection

### Tooling
- [ ] `-O cost / precision / latency / local` compiler flags
- [ ] `lagent.toml` project file parsing
- [ ] `lagent fmt` (auto-formatter)

---

## Phase 3 — Advanced Features (Months 7–9)
Target: complete kernel support, interrupts, and resource safety.

### Language
- [ ] Full `kernel` blocks with `observe`, `reason`, `act`, `verify`
- [ ] `verify` retry loop with `MAX_KERNEL_RETRIES`
- [ ] `interruptible` blocks and Safe Interaction Points
- [ ] `ctx_compress` and `ctx_share` primitives

### VM
- [ ] Checkpointing for `interruptible` blocks
- [ ] GPU swap and memory quotas for local models
- [ ] `--deterministic` mode (temperature=0)
- [ ] Semantic logging and replay

---

## Phase 4 — Maturity and Tooling (Month 10+)
Target: production-quality ecosystem.

### Tooling
- [ ] `lagent-lsp`: LSP server (auto-completion, diagnostics)
- [ ] `lagent-dbg`: interactive debugger with context inspection
- [ ] Package manager (`lagent add`, `lagent publish`)

### Interoperability
- [ ] Python bindings via `PyO3`
- [ ] NERD intermediate format as optional compiler target
- [ ] FFI Agentique for calling from Rust/JS

### Documentation
- [ ] Language tour and tutorials
- [ ] API reference
- [ ] Example agent library

---

## Tracking

Progress is tracked in GitHub Issues. Each roadmap item maps to an issue with a corresponding milestone.

| Milestone | Description         | Target     |
|-----------|---------------------|------------|
| v0.1      | Phase 1 complete    | Month 3    |
| v0.2      | Phase 2 complete    | Month 6    |
| v0.3      | Phase 3 complete    | Month 9    |
| v1.0      | Phase 4 complete    | Month 12+  |
