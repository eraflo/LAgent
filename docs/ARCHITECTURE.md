# L-Agent Architecture

## Design Philosophy

**C gave you control over silicon. L-Agent gives you control over language models.**

In systems programming, C exposes hardware primitives — registers, memory, interrupts — and lets the programmer manage them explicitly. L-Agent applies the same philosophy to LLMs:

| C Concept | L-Agent Equivalent |
|-----------|-------------------|
| `malloc` / `free` | `ctx_alloc` / `ctx_free` — explicit context window management |
| CPU registers | Logits, token budgets, inference parameters |
| Interrupts | `interruptible` blocks — safe interaction points |
| Function calls | `fn`, `skill`, `spell`, `kernel` — stratified by capability |
| `#pragma` | `#[attribute(...)]` — compiler directives |

The language is **compiled to bytecode** and executed on a **stack-based VM**, both written in Rust. This gives you:

- **Static guarantees** — type checking, name resolution, constraint validation at compile time
- **Explicit resource control** — you manage context windows, not the runtime
- **Backend abstraction** — swap models without changing code
- **Deterministic testing** — simulated backend for reproducible execution

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        lagent CLI                               │
│                   (lagent-cli crate)                            │
│                                                                 │
│  Commands: build │ run │ check │ fmt                            │
└────────────┬──────────────────────────────┬─────────────────────┘
             │                              │
    compile  │                              │  execute
             ▼                              ▼
┌────────────────────────┐    ┌──────────────────────────────────┐
│   lagent-compiler      │    │         lagent-vm                │
│                        │    │                                  │
│  lexer  (logos)        │    │  Vm                              │
│    │                   │    │   ├─ TokenHeap        (heap)     │
│  parser (chumsky)      │    │   ├─ KernelTable                 │
│    │                   │    │   ├─ MemorySlots                 │
│  resolver              │    │   ├─ LoreTable                    │
│    │                   │    │   ├─ InferenceBackend            │
│  semantic analysis     │    │   │   ├─ SimulatedBackend        │
│    │                   │    │   │   └─ AnthropicBackend  [*]   │
│  codegen → .lbc        │    │   └─ OpCode dispatcher           │
│                        │    │                                  │
│  lexer/mod.rs          │    │  vm.rs                           │
│  parser/mod.rs         │    │  runtime/token_heap.rs           │
│  resolver.rs           │    │  backends/mod.rs                 │
│  semantic/mod.rs       │    │  persistent_store.rs             │
│  codegen/mod.rs        │    │                                  │
└────────────────────────┘    └──────────────────────────────────┘

  [*] feature-gated: --features backend-remote
```

---

## Crate Responsibilities

### `lagent-compiler` — `.la` → `.lbc` Bytecode

Transforms L-Agent source into serialized bytecode through a five-stage pipeline.

#### Pipeline

```
Source .la
    │
    ▼
┌─────────────┐  Vec<Token>
│   Lexer     │  (logos)
└──────┬──────┘
       │
       ▼
┌─────────────┐  Vec<Item>  (AST)
│   Parser    │  (chumsky combinators)
└──────┬──────┘
       │
       ▼
┌─────────────┐  Vec<Item>  (imports inlined)
│  Resolver   │  recursive, depth-first
└──────┬──────┘
       │
       ▼
┌─────────────┐  TypedAst { items, type_env, oracle_names, lore }
│  Semantic   │  name resolution + type checking
└──────┬──────┘
       │
       ▼
┌─────────────┐  Vec<u8>  (bincode-serialized)
│  Codegen    │  3-pass bytecode emission
└─────────────┘
    │
    ▼
  .lbc  (magic: b"LAGN")
```

**Entry points:**
- `compile(src: &str) -> Result<Vec<u8>>` — in-memory, no filesystem access
- `compile_file(path: &Path) -> Result<Vec<u8>>` — reads source, resolves imports, full pipeline

#### Stages in Detail

| Stage | Source | Input | Output | Key Files |
|-------|--------|-------|--------|-----------|
| **Lexer** | `logos` token definitions | Source text | `Vec<Token>` | `src/lexer/mod.rs` |
| **Parser** | `chumsky` parser combinators | `Vec<Token>` | `Vec<Item>` (AST) | `src/parser/mod.rs`, `src/parser/ast.rs` |
| **Resolver** | Recursive import expansion | `Vec<Item>` | `Vec<Item>` (flattened, imports inlined) | `src/resolver.rs` |
| **Semantic** | Name resolution + type checking | `Vec<Item>` | `TypedAst` | `src/semantic/mod.rs` |
| **Codegen** | 3-pass bytecode emission | `TypedAst` | `Vec<u8>` (bincode) | `src/codegen/mod.rs`, `src/codegen/opcodes.rs` |

#### Codegen — Three Passes

| Pass | Items Processed | Output |
|------|----------------|--------|
| **0 — Lore** | `LoreDecl` | `StoreLore` opcodes prepended to main stream |
| **1 — Kernels** | `KernelDef`, `SpellDef`, `SkillDef` | `KernelBytecode` table (callable via `CallKernel`) |
| **2 — Main** | `FnDef`, `MemoryDecl` | Flat instruction stream (soul preamble before `fn main`) |

Skills, spells, and kernels are all compiled into the same kernel table and dispatched via `CallKernel`. The soul preamble (`SetAgentMeta` + `CtxAppendLiteral`) is injected before the body of `fn main`.

#### Bytecode Format

`.lbc` files use the magic header `b"LAGN"` and contain:
- Kernel bytecode table (indexed kernels with bytecode bodies)
- Flat instruction stream (soul preamble + main function body)
- Serialized via `bincode` for fast deserialization

---

### `lagent-vm` — Bytecode Execution

A stack-based virtual machine that executes `.lbc` bytecode and manages LLM inference.

#### Core Components

| Component | File | Responsibility |
|-----------|------|----------------|
| **`Vm`** | `vm.rs` | Main execution loop, opcode dispatch, frame management |
| **`TokenHeap`** | `runtime/token_heap.rs` | Slab allocator for context segments (analogous to `malloc`/`free`) |
| **`InferenceBackend`** | `backends/mod.rs` | Trait abstracting LLM interaction (infer, classify, compress, act, oracle) |
| **`PersistentStore`** | `persistent_store.rs` | Trait for cross-run persistence (JSON file-backed by default) |

#### VM State

```rust
pub struct Vm {
    heap: TokenHeap,                    // context segment allocator
    backend: Box<dyn InferenceBackend>, // LLM interaction
    soul_meta: Option<String>,          // agent identity (from soul block)
    memory: HashMap<String, Value>,     // named intra-run memory slots
    lore: HashMap<String, String>,      // static knowledge strings
    kernel_table: Vec<KernelBytecode>,  // compiled kernels/spells/skills
    call_stack: Vec<Frame>,             // execution frames
}
```

#### Execution Model

The VM uses **stack-based evaluation** with **kernel call frames**:

1. **Stack machine** — values are pushed/popped; locals are named entries in the current frame
2. **Kernel frames** — calling a kernel pushes a new frame with bound parameters
3. **Verify retry** — if `VerifyStep` fails inside a kernel, the frame is re-executed from the start (up to `MAX_KERNEL_RETRIES`)
4. **Interruptible checkpoints** — `BeginInterruptible` saves a frame checkpoint; errors inside restore it

#### Memory Model

| Layer | Scope | Lifetime | Mechanism |
|-------|-------|----------|-----------|
| **Stack locals** | Per-frame | Frame lifetime | `StoreLocal` / `LoadLocal` |
| **Memory slots** | Global (intra-run) | Program lifetime | `AllocMemorySlot` / `StoreMemory` / `LoadMemory` |
| **Persistent memory** | Global (inter-run) | Across restarts (file-backed) | `PersistentStore` trait |
| **Lore table** | Global (read-only) | Program lifetime | `StoreLore` / `LoadLore` |
| **Token Heap** | Explicit | Until `ctx_free` | `TokenHeap` slab allocator |

#### InferenceBackend Trait

```rust
pub trait InferenceBackend {
    fn infer(&mut self, prompt: &str) -> Result<String>;
    fn classify(&mut self, prompt: &str, labels: &[String]) -> Result<String>;
    fn compress(&mut self, text: &str) -> Result<String>;
    fn act(&mut self, payload: &str) -> Result<String>;
    fn oracle(&mut self, name: &str, args: &[String]) -> Result<String>;
}
```

| Implementation | Feature Flag | Description |
|----------------|-------------|-------------|
| **`SimulatedBackend`** | default | Deterministic, no model required — returns structured placeholders |
| **`AnthropicBackend`** | `backend-remote` | HTTP via `reqwest` to Anthropic Messages API |

Adding a new backend requires implementing all five methods and gating it behind a Cargo feature flag. See *Extending Backends* below.

---

### `lagent-cli` — User Interface

Thin binary wrapping the compiler and VM. Provides `build`, `run`, `check`, and `fmt` commands.

| Command | Action |
|---------|--------|
| `lagent build [input]` | Compile `.la` → `.lbc` (or `.lalb` with `--lib`) |
| `lagent run [input]` | Compile + execute |
| `lagent check [input]` | Syntax + semantic analysis only (no codegen) |
| `lagent fmt [input]` | Auto-format source in place |
| `lagent fmt --check [input]` | Check formatting without modifying |

**Runtime flags for `run`:**

| Flag | Description |
|------|-------------|
| `--backend simulated\|anthropic` | Select inference backend (default: `simulated`) |
| `--deterministic` | Temperature=0 for reproducible inference |
| `--context N` | Token heap size (default: 4096) |
| `--persist <path>` | Attach file-backed persistent store for cross-run memory |

---

## Data Flow

### Compilation Path

```
src/main.la
    │  read_to_string
    ▼
lagent_compiler::compile_file(path)
    │  lexer::tokenize    →  Vec<Token>
    │  parser::parse      →  Vec<Item>        (AST)
    │  resolver::resolve_uses  →  Vec<Item>   (imports inlined)
    │  semantic::analyze  →  TypedAst         (type env, oracle names, lore)
    │  codegen::generate  →  Vec<u8>          (.lbc bytes, bincode)
    │    Pass 0: StoreLore opcodes
    │    Pass 1: KernelBytecode table (kernels + spells + skills)
    │    Pass 2: Main stream (soul preamble + fn main body)
    ▼
main.lbc  (written to disk)
```

### Execution Path

```
main.lbc
    │  bincode::deserialize
    ▼
lagent_vm::Vm::execute(bytecode)
    │  deserialize → Bytecode { kernels, instructions }
    │  loop: dispatch each OpCode
    │    StoreLore       → vm.lore.insert(...)
    │    AllocMemorySlot → vm.memory.insert(...)
    │    SetAgentMeta    → vm.soul_meta = Some(...)
    │    CtxAlloc        → TokenHeap::alloc
    │    CallKernel      → push kernel frame, execute body, pop frame
    │    BranchClassify  → InferenceBackend::classify
    │    CallOracle      → InferenceBackend::oracle
    │    VerifyStep      → retry on failure (up to MAX_KERNEL_RETRIES)
    ▼
  stdout / side effects
```

---

## Token Heap

The Token Heap is L-Agent's equivalent of the C memory heap — but instead of bytes, it manages **LLM context tokens**.

See [`TOKEN_HEAP.md`](TOKEN_HEAP.md) for the complete design document.

**Quick summary:**

| C Concept | Token Heap Equivalent |
|-----------|----------------------|
| `malloc(size)` | `ctx_alloc(tokens) → segment_id` |
| `free(ptr)` | `ctx_free(segment_id)` |
| Memory leak | Context token exhaustion → `CTX_OVERFLOW` |
| `realloc` | `ctx_resize(segment_id, new_tokens)` |

**Current implementation:** Slab allocator in `lagent-vm/src/runtime/token_heap.rs`:
- O(n) allocation (linear scan for free slot)
- O(1) free by id
- Tracks `total_capacity` vs `used` to prevent overflow
- Each `CtxSegment` has an `id`, `capacity`, and `content` string

**Planned extensions (Phases 10-12):** Context views, copy-on-write, memory-mapped contexts, context swapping, versioning, semantic GC, diff, pagination, merge, clear, inspection (`ctx_len`, `ctx_capacity`).

---

## Project Structure

```
lagent/
├── lagent-compiler/         # .la → bytecode compiler
│   └── src/
│       ├── lexer/           # logos-based tokeniser
│       ├── parser/          # chumsky parser → AST
│       │   ├── mod.rs       # parser combinators
│       │   └── ast.rs       # AST node definitions
│       ├── codegen/         # 3-pass bytecode generator
│       │   ├── mod.rs       # codegen orchestration
│       │   └── opcodes.rs   # OpCode enum definitions
│       ├── semantic/        # name resolution + type checking
│       ├── resolver.rs      # module import expansion (pub visibility)
│       ├── project.rs       # lagent.toml manifest parsing
│       └── fmt.rs           # AST pretty-printer (lagent fmt)
│
├── lagent-vm/               # bytecode executor
│   └── src/
│       ├── vm.rs            # stack-based VM, opcode dispatch
│       ├── backends/        # InferenceBackend trait + implementations
│       │   └── mod.rs       # SimulatedBackend, AnthropicBackend
│       ├── persistent_store.rs  # PersistentStore trait + FilePersistentStore
│       └── runtime/         # runtime primitives
│           └── token_heap.rs    # TokenHeap slab allocator
│
├── lagent-cli/              # lagent build/run/check/fmt binary
│   └── src/
│       └── main.rs          # CLI entry point (clap)
│
├── examples/                # example .la programs
├── docs/                    # documentation
│   ├── ARCHITECTURE.md      # this file
│   ├── SPEC.md              # language specification
│   ├── ROADMAP.md           # development roadmap
│   └── TOKEN_HEAP.md        # token heap design document
│
├── Cargo.toml               # workspace manifest
├── lagent.toml              # example project manifest
└── README.md                # project overview
```

---

## Extending the Compiler

Adding a new language feature requires changes across **7 layers**:

| Step | File | What to Add |
|------|------|-------------|
| 1 | `lexer/mod.rs` | `#[token("keyword")] Keyword` token variant |
| 2 | `parser/ast.rs` | AST node struct or enum variant |
| 3 | `parser/mod.rs` | Parser combinator rule + wire into `item()` / `stmt()` |
| 4 | `semantic/mod.rs` | Name registration, type validation, env updates |
| 5 | `codegen/opcodes.rs` | New `OpCode` variant(s) |
| 6 | `codegen/mod.rs` | Emission logic from AST nodes to opcodes |
| 7 | `vm.rs` | Opcode dispatch arm + execution logic |

**Example: adding `const`:**
1. `#[token("const")] Const` in lexer
2. `ConstDef { name, ty, value }` in AST
3. `just(Token::Const).ignore_then(...)` in parser
4. Evaluate at compile-time, insert into const env in semantic
5. No runtime opcode needed — constants are inlined at codegen
6. Inline the evaluated value where the const is referenced
7. No VM changes

**Quality gates** (all must pass before merging):
```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
```

---

## Extending Backends

Implement the `InferenceBackend` trait:

```rust
pub trait InferenceBackend {
    fn infer(&mut self, prompt: &str) -> Result<String>;
    fn classify(&mut self, prompt: &str, labels: &[String]) -> Result<String>;
    fn compress(&mut self, text: &str) -> Result<String>;
    fn act(&mut self, payload: &str) -> Result<String>;
    fn oracle(&mut self, name: &str, args: &[String]) -> Result<String>;
}
```

1. Create a new struct in `lagent-vm/src/backends/`
2. Implement all five methods
3. Gate it behind a Cargo feature flag in `Cargo.toml`
4. Add conditional compilation in `backends/mod.rs`
5. Update the CLI to accept the new backend name in `--backend`

---

## Error Handling

### Compile-Time Errors

| Error Type | Source | Description |
|------------|--------|-------------|
| Lexical errors | Lexer | Invalid characters, unterminated strings |
| Parse errors | Parser | Syntax violations, mismatched brackets |
| Name resolution | Semantic | Undefined identifiers, duplicate names |
| Type errors | Semantic | Type mismatches, invalid semantic type usage |
| Constraint errors | Semantic | Unknown constraint names, invalid apply targets |

### Runtime Errors

| Error Type | Source | Recovery |
|------------|--------|----------|
| `HeapError::Overflow` | TokenHeap | Context budget exceeded — fatal |
| `HeapError::InvalidHandle` | TokenHeap | Use-after-free or invalid segment — fatal |
| `KernelVerifyError` | VM | Kernel verify failed after all retries — fatal |
| `ConstraintViolation` | VM | Constraint check failed — non-retriable, fatal |
| Backend errors | InferenceBackend | Network failures, API errors — may be retried |

---

## Feature Flags

| Flag | Description | Dependencies |
|------|-------------|-------------|
| *(default)* | Simulated backend only | — |
| `backend-remote` | Enable Anthropic API | `reqwest`, `tokio` |

Workspace-level:
```toml
[workspace.dependencies]
logos = "0.14"          # lexer
chumsky = "0.9"         # parser combinators
serde = "1"             # bytecode serialization
toml = "0.8"            # lagent.toml parsing
clap = "4"              # CLI argument parsing
thiserror = "1"         # error types
anyhow = "1"            # error handling
```

---

## Testing Strategy

| Level | Scope | Location |
|-------|-------|----------|
| Unit tests | Individual components | `#[cfg(test)]` modules in each `.rs` file |
| Integration tests | Full compile → run pipeline | `tests/` directory |
| Example programs | End-to-end validation | `examples/*.la` |
| Backend tests | Backend implementations | `backends/mod.rs` test module |

Example programs serve as both documentation and regression tests:
- `examples/hello.la` — basic compilation and execution
- `examples/agent_soul.la` — soul, skill, lore, memory, oracle, constraint
- `examples/kernel_call.la` — callable kernels, verify retry, interruptible
- `examples/emotion_analysis.la` — semantic types, kernels, probabilistic branching
