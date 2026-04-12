# L-Agent Development Roadmap

## Phase 1 — Proof of Concept ✅
Target: a working pipeline from source to execution for a minimal subset.

### Compiler
- [x] `logos`-based lexer
- [x] `chumsky`-based parser for `fn`, `let`, `ctx_alloc`, `ctx_free`, `ctx_append`, `println`
- [x] Semantic analysis: basic name resolution and primitive type checking
- [x] Bytecode emission and `bincode` serialization

### VM
- [x] OpCode dispatcher loop
- [x] `CtxAlloc`, `CtxFreeStack`, `CtxAppendStack` opcodes
- [x] `Println`, `Halt`, `PushStr`, `StoreLocal`, `LoadLocal` opcodes
- [x] `TokenHeap` for context segment management

### Tooling
- [x] `lagent build` and `lagent run` commands
- [x] `lagent check` for syntax errors
- [x] Basic test suite (`examples/hello.la`)

---

## Phase 2 — Semantic Types & Kernels ✅
Target: probabilistic branching, semantic types, and reasoning kernels.

### Language
- [x] `type Name = semantic(...)` declaration and type alias resolution
- [x] `branch` / `case` / `default` syntax and semantics
- [x] Full `kernel` blocks with `observe`, `reason`, `act`, `verify`
- [x] `infer` with semantic type label injection (`InferClassify`)
- [x] `BinOp` expressions (`!=`, `>`, `<`)

### VM
- [x] `BranchClassify` opcode with constrained decoding simulation
- [x] `InferClassify` with label list
- [x] `Observe`, `Act`, `VerifyStep` opcodes
- [x] `CallKernel` opcode with `KernelBytecode` table
- [x] `SimulatedBackend` for deterministic testing

---

## Phase 3 — Kernel Frames & Resource Safety ✅
Target: complete kernel support, interrupts, and resource safety.

### Language
- [x] Kernel call frames: parameters bound as locals in kernel scope
- [x] `verify` retry loop up to `MAX_KERNEL_RETRIES` (default: 3)
- [x] `interruptible` blocks and safe interaction points
- [x] `ctx_compress` primitive (context summarization)

### VM
- [x] Kernel call frames: push/pop frame, bind params, restore on return
- [x] `VerifyStep` retry with `KernelVerifyError` on exhaustion
- [x] `BeginInterruptible` / `EndInterruptible` with checkpoint save/restore
- [x] `CtxCompress` opcode (delegates to `InferenceBackend::compress`)

---

## Phase 4 — Agent Vocabulary & Module System ✅
Target: L-Agent's distinct identity keywords, module imports, remote backend.

### Language — Agent Vocabulary
- [x] `soul { instruction "..."; }` — agent identity block; preamble injected before `fn main`
- [x] `skill Name(params) -> T { body }` — callable capability (compiled into kernel table)
- [x] `instruction "text";` statement — appends literal to active context handle
- [x] `spell Name(params) -> T { body }` — multi-step workflow (compiled like `kernel`)
- [x] `memory Name: T = expr;` — named persistent slot, survives context resets
- [x] `oracle Name(params) -> T;` — external lookup stub; VM calls `backend.oracle()`
- [x] `constraint Name { verify(expr); }` — named guard block (inlined at call site in Phase 5)
- [x] `lore Name = "text";` — named static knowledge string
- [x] `ctx_share` built-in — duplicate a context handle reference

### Module System
- [x] `use "path.la";` — inline module expansion before semantic analysis
- [x] `pub` modifier parsed on top-level items (enforcement deferred to Phase 5)
- [x] `resolver.rs` — recursive `resolve_uses()` expands imports transitively
- [x] `compile_file()` — filesystem-aware entry point (used by CLI)

### VM — New Opcodes
- [x] `SetAgentMeta(String)` — stores soul metadata string
- [x] `CtxAppendLiteral(String)` — appends literal to in-scope ctx handle
- [x] `RegisterSkill(String)` — metadata marker (no-op at runtime)
- [x] `AllocMemorySlot(String)` / `LoadMemory(String)` / `StoreMemory(String)`
- [x] `CallOracle(String, u8)` — dispatches to `InferenceBackend::oracle()`
- [x] `BeginConstraint(String)` / `EndConstraint` — no-op in Phase 4
- [x] `StoreLore(String, String)` / `LoadLore(String)`
- [x] `CtxShare` — duplicates TOS ctx handle

### Remote Backend
- [x] `oracle()` method added to `InferenceBackend` trait
- [x] `AnthropicBackend` (feature-gated: `backend-remote`) via `reqwest`
- [x] `--backend simulated|anthropic` CLI flag
- [x] `--deterministic` CLI flag (temperature=0)

---

## Phase 5 — Constraint Enforcement & Visibility (Next)
Target: `constraint` inlining at call sites, `pub` visibility, `lagent.toml`, persistent memory.

### Language
- [ ] `constraint` bodies inlined at call site in codegen
- [ ] `pub` visibility enforcement (private items not exported across module boundaries)
- [ ] `lagent.toml` project file (`[lib]` entry, `name`)
- [ ] `.lalb` (L-Agent Library Bundle): precompiled bytecode + export table
- [ ] `memory_load` / `memory_save` / `memory_delete` built-in primitives

### VM
- [ ] `BeginConstraint` / `EndConstraint` — enforce guard at runtime (non-retriable error)
- [ ] Persistent memory backend (optional file-backed store)

### Tooling
- [ ] `lagent build --lib` produces `.lalb`
- [ ] `lagent add <lib>` installs from a registry
- [ ] `lagent fmt` — auto-formatter

---

## Phase 6 — Maturity & Tooling
Target: production-quality ecosystem.

### Compiler
- [ ] `-O cost / precision / latency / local` optimisation flags
- [ ] Dead-code elimination for unused kernels and skills

### Tooling
- [ ] `lagent-lsp`: LSP server (auto-completion, hover, diagnostics)
- [ ] `lagent-dbg`: interactive debugger with context inspection
- [ ] Package manager (`lagent add`, `lagent publish`)

### Interoperability
- [ ] Python bindings via `PyO3`
- [ ] FFI for calling from Rust/JS
- [ ] NERD intermediate format as optional compiler target

### Documentation
- [ ] Language tour and tutorials
- [ ] API reference
- [ ] Example agent library

---

## Milestones

| Milestone | Description                    | Status      |
|-----------|--------------------------------|-------------|
| v0.1      | Phase 1 — Proof of Concept     | ✅ Complete |
| v0.2      | Phase 2 — Semantic Types       | ✅ Complete |
| v0.3      | Phase 3 — Kernel Frames        | ✅ Complete |
| v0.4      | Phase 4 — Agent Vocabulary     | ✅ Complete |
| v0.5      | Phase 5 — Constraints & Modules| In progress |
| v1.0      | Phase 6 — Production           | Planned     |
