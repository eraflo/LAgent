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
- [x] `constraint Name { verify(expr); }` — named guard block (inlined at call site in Phase 6)
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

## Phase 5 — Sémantique Stricte des Primitives Agentiques
Target: fundamentally distinguish `skill`, `spell`, and `kernel` with compile-time guarantees and runtime isolation.

### fn — Fonction Pure (Non Agentique)
- [ ] **Compile-time:** forbid `ctx_*`, `infer`, `branch`, `observe`, `reason`, `act`, `verify`, calls to `kernel` or `spell` — calls only to other `fn` or `skill`
- [ ] **Runtime:** standard function call, no LLM interaction, no context manipulation
- [ ] Pure computation — equivalent of a classic programming function
- [ ] Distinction with `skill`: `fn` = computation classique, `skill` = peut être annoté `#[tool]` pour function calling LLM

### skill — Pure Function, No LLM, No Context
- [ ] **Compile-time:** forbid `ctx_*`, `infer`, `branch`, `observe`, `reason`, `act`, `verify` — calls only to other `skill` or pure `fn`
- [ ] **Runtime:** standard call, bounded execution time, no blocking I/O
- [ ] `#[tool]` attribute — safe exposure for LLM function calling (OpenAI-style tools)
- [ ] Compiler guarantees: no context leakage, no hidden cost, callable by external models safely
- [ ] Equivalent of "tools" but **guaranteed by the compiler**, not just by convention

### spell — Sequential Orchestrated Workflow
- [ ] **Compile-time:** no particular restriction — all primitives allowed
- [ ] **Runtime:** linear execution, no implicit retry, no imposed structure
- [ ] Can allocate/free context, call `infer`, `branch`, `skill`
- [ ] Compiler may optimise (e.g., prompt fusion, batch inference calls)
- [ ] Default granularity for composing agent behaviors

### kernel — Transactional Reasoning with Isolation
- [ ] **Compile-time:** exactly 4 sections required in order — `observe` → `reason` → `act` → `verify`; `return` only in `act`; no side effects outside `act`
- [ ] **Runtime:** allocates a private `CtxSegment` — total isolation from caller context
- [ ] `verify` failure → full rollback and re-execution from `observe` (up to N retries)
- [ ] Success → optional merge of private context back into caller's context
- [ ] "Cognitive transaction" with automatic rollback — analogous to a database transaction

### VM — New Opcodes
- [ ] `EnterKernelIsolation()` — allocate private context segment, save caller state
- [ ] `CommitKernel()` — merge private context into parent, restore caller state
- [ ] `RollbackKernel()` — discard private context, restore pre-isolation state
- [ ] `CheckSkillSafety()` — compile-time pass ensuring skill body contains no forbidden primitives

---

## Phase 6 — Constraint Enforcement & Visibility ✅
Target: `constraint` inlining at call sites, `pub` visibility, `lagent.toml`, persistent memory.

### Language
- [x] `apply ConstraintName;` statement — inline constraint body at call site in codegen
- [x] `ConstraintVerify` opcode — non-retriable guard (distinct from kernel `VerifyStep`)
- [x] `pub` visibility enforcement (private items not exported across module boundaries)
- [x] `lagent.toml` project file (`[lib]` entry, `name`) — `project.rs`
- [x] `.lalb` (L-Agent Library Bundle): precompiled bytecode + export table — `LibraryBundle`
- [x] `memory_load` / `memory_save` / `memory_delete` built-in primitives (persistent cross-run)

### VM
- [x] `ConstraintVerify` — non-retriable `ConstraintViolation` error (no retry loop)
- [x] `PersistLoad` / `PersistSave` / `PersistDelete` opcodes
- [x] `PersistentStore` trait + `FilePersistentStore` (JSON, atomic write-then-rename)
- [x] `InMemoryPersistentStore` for testing
- [x] `Vm::with_persistent_store()` builder

### Tooling
- [x] `lagent build --lib` produces `.lalb`
- [x] `lagent build` auto-discovers `lagent.toml` when no input file is given
- [x] `lagent run --persist <path>` attaches file-backed persistent store
- [x] `lagent fmt` — auto-formatter (round-trip AST pretty-printer)
- [x] `lagent fmt --check` — exit non-zero if file would change
- [ ] `lagent add <lib>` installs from a registry (deferred to Phase 8)

---

## Phase 7 — Fondamentaux du Langage (partielle)
Target: essential control structures, composite types, collections, error handling, and mutability.

### Control Flow
- [x] `loop { ... }` — infinite loop with `break` and `continue`
- [x] `while condition { ... }` — conditional loop
- [x] `for item in vec { ... }` — iteration over `Vec<T>` collections (index-based, works with `break`/`continue`)
- [x] `if condition { ... } else { ... }` — deterministic boolean branching (distinct from probabilistic `branch`)

### Composite Types
- [x] `struct Name { field: Type, ... }` — named field aggregates (declaration + construction `Name { field: expr }` + field access `s.field`)
- [x] `enum Name { Variant, Variant(T) }` — tagged unions with optional variant payloads (declaration + construction `Variant(expr)` / `Variant`)
- [x] Tuples — `(T, U, V)` anonymous product types, tuple indexing `tuple.0`

### Collections
- [x] `Vec<T>` — dynamic arrays with literals `[a, b, c]`, indexing `arr[i]`
- [ ] `Map<K, V>` — key-value dictionaries with literals `{key: val}`
- [ ] `Set<T>` — unordered unique-element collections

### Mutability & Constants
- [x] `let mut x = ...` — explicit mutable binding (immutable `let` by default)
- [x] Reassignment `x = expr;` — only allowed on `mut` bindings (compile-time error otherwise)
- [x] `const NAME: Type = value;` — **évaluée à la compilation** (littéraux + opérations binaires, références entre constantes)
- [x] `const` is **never visible par le LLM** — pure code-level value (calculs, comparaisons, conditions)
- [x] Distinction sémantique : `const` → programmation, `lore` → connaissances injectées dans le contexte système du LLM

### Type Safety
- [x] Rejet compile-time : arithmetic sur strings (`"a" + 1`)
- [x] Rejet compile-time : logique sur non-booleans (`1 && 5`)
- [x] Rejet compile-time : appel de fonction undefined
- [x] Rejet compile-time : `break`/`continue` hors boucle
- [x] Runtime : division par zéro, index vector out of bounds, field access invalide

### Persistence Unification
- [ ] `persistent memory Name: T = expr;` — nouvelle syntaxe pour persistance **inter-run** (sauvegardé dans le fichier via `--persist`)
- [ ] `memory Name: T = expr;` existant → renommé conceptuellement en persistance **intra-run** (survit aux kernels, pas aux redémarrages)
- [ ] `memory_load` / `memory_save` / `memory_delete` dépréciés comme primitives utilisateur — remplacés par accès direct aux slots `persistent memory`
- [ ] Le compilateur génère automatiquement la sérialisation/désérialisation pour les `persistent memory`

### Compiler Tightening
- [ ] `instruction` restreint au bloc `soul` uniquement — erreur de compilation si utilisé ailleurs (restriction du comportement Phase 4)

### Bytecode Cleanup
- [ ] `PersistLoad` / `PersistSave` / `PersistDelete` — supprimés du bytecode de surface, deviennent détails d'implémentation VM interne (gestion automatique par `persistent memory`)
- [ ] `RegisterSkill` — supprimé, metadata sans effet runtime ; la table des kernels contient déjà les noms pour le débogage
- [ ] `AllocMemorySlot` / `LoadMemory` / `StoreMemory` — conservés pour `memory` intra-run

### Error Handling
- [ ] `try { ... } catch { ... }` or `Result<T, E>` type with `?` propagation operator
- [ ] Integration with planned `AgentError` type (Phase 8)
- [ ] `throw expr` — explicit error raising

### Extended Operators
- [x] Arithmetic: `+`, `-`, `*`, `/`, `%` (Int + Float supportés, types mixtes rejetés au runtime)
- [x] Logical: `&&`, `||` (opérandes booléennes requises)
- [x] Equality: `==` (symmetric with existing `!=`)
- [x] Arithmetic opcodes in VM: `Add`, `Sub`, `Mul`, `Div`, `Mod`
- [x] Vector opcodes in VM: `VecNew`, `VecGet`, `VecSet`, `VecLen`, `VecPush`
- [x] Tuple/Struct opcodes in VM: `TuplePack`, `FieldAccess`, `StructConstruct`, `EnumVariant`

---

## Phase 8 — Robustesse & Expressivité
Target: AI-aware error handling, fallback strategies, and cost tracking.

### Language — Probabilistic Programming
- [ ] `Dist<T>` type — wraps `Vec<(T, f32)>` with operators `most_likely()`, `sample()`, `filter(prob > x)`
- [ ] `AgentError` type — dedicated error type with AI-specific variants: `ToolNotFound`, `IntentAmbiguous`, `ModelRefusal`, `ContextOverflow`
- [ ] Fallback strategies — `branch { ... } fallback { ... }` executed when no branch matches
- [ ] Model chaining — `model: "gpt-5" || "claude-4" || "llama-3-local"` for automatic fallback
- [ ] Extended contracts — pre/post-conditions on `constraint` with remediation: `on_violate { ... }`
- [ ] `token_cost() -> u64` — built-in function returning cumulative token cost of execution
- [ ] `tokens_used() -> u64` — exact token consumption since program start
- [ ] `tokens_limit() -> u64` — remaining token budget for current execution context

### Tooling
- [ ] `lagent-lsp`: LSP server (auto-completion, hover, diagnostics)
- [ ] `lagent-dbg`: interactive debugger with context inspection
- [ ] Better error messages with `AgentError` variant suggestions
- [ ] VS Code extension — syntax highlighting (`.la`), code snippets, build/run tasks, integrated terminal, problem matchers

---

## Phase 9 — Parallélisme & Contrôle des Ressources
Target: concurrent execution, fine-grained token budgeting, and compiler optimisations.

### Language — Parallelism
- [ ] `parallel { ... }` block — execute multiple skills/kernels concurrently, merge results
- [ ] `race { ... }` block — execute in parallel, take first completed result, cancel others
- [ ] Configurable merge strategies for `parallel`: `merge = first`, `merge = majority_vote`, `merge = custom_fn(...)` — explicit developer control over result fusion (underlying primitive: `ctx_merge`, Phase 12)
- [ ] Concurrent write conflict resolution — semantic locks, merge strategies for agent state
- [ ] `ctx.with_budget(max_tokens: u32) { ... }` — token budget scoping for critical blocks

### Compiler
- [ ] `-O cost` — favour cheapest models
- [ ] `-O precision` — favour most performant models
- [ ] `-O latency` — optimise for response time
- [ ] `-O local` — force exclusive local model usage
- [ ] Dead-code elimination for unused kernels and skills

---

## Phase 10 — Méta-programmation & Génériques
Target: user-defined abstractions, type-level safety, and higher-order programming.

### Language — Macros
- [ ] Declarative macro system — `macro!` syntax for user-defined abstractions
- [ ] `define_agent!` macro — standard agent skeleton generator
- [ ] Hygienic macro expansion in codegen

### Language — Generics
- [ ] Semantic type generics — `skill classify<T: semantic>(input: str) -> T { ... }`
- [ ] Generic constraints on semantic labels
- [ ] Type inference for generic callsites
- [ ] Generic data structures — `List<T>`, `Option<T>`, `Result<T, E>`

### Language — Attributes System
- [ ] Custom attribute syntax `#[name(key = value, ...)]` — user-definable metadata on functions/kernels/types
- [ ] `#[prompt("system")]`, `#[prompt("user")]`, `#[prompt("assistant")]` — built-in attributes controlling prompt assembly role on functions/kernels
- [ ] Attribute validation in semantic analysis
- [ ] Attribute propagation through module boundaries
- [ ] Foundation for `#[capability(...)]` (Phase 13) and `#[budget(...)]` (Phase 13)

### Language — Higher-Order Functions
- [ ] Lambdas/closures — `|x| x + 1`, `|a, b| a + b`
- [ ] Functions as first-class values — pass functions as arguments, return them
- [ ] `map`, `filter`, `fold` on collections
- [ ] Callback patterns and event handlers

### Operator Overloading
- [ ] `impl Add for Vec` — implement operators for user-defined types
- [ ] `impl Eq`, `impl Ord` for struct/enum types
- [ ] Operator trait definitions in standard library

### Standard Library
- [ ] HTTP client — `http::get(url)`, `http::post(url, body)`
- [ ] Data parsing — `json::parse()`, `csv::read()`, `markdown::parse()`
- [ ] NLP utilities — `sentiment(text)`, `summarize(text)`, `translate(text, lang)`

---

## Phase 11 — Interopérabilité & Écosystème
Target: external world integration, package distribution, and fine-grained visibility.

### Interoperability
- [ ] FFI specification — C ABI for cross-language calling
- [ ] Python bindings via `PyO3`
- [ ] JS/TS bindings
- [ ] Rust integration — embed L-Agent VM in Rust applications
- [ ] Distinguer `oracle` (appels externes médiés par le backend, non-déterministes : API, DB, capteurs) de `extern` (FFI directe vers code natif Rust/C, déterministe)
- [ ] `oracle` peut être implémenté par des plugins backend utilisateur

### Model Context Protocol
- [ ] `use_mcp("server_url")` — native MCP server connection
- [ ] MCP tool discovery and invocation
- [ ] MCP resource access

### Visibility
- [ ] `pub(crate)` — visible within current compilation unit only
- [ ] `pub(super)` — visible within parent module
- [ ] `pub(self)` — explicit self-module visibility (alias for private)
- [ ] `pub(in path)` — arbitrary scoped visibility

### Ecosystem
- [ ] NERD intermediate format as optional compiler target
- [ ] Package manager — `lagent add`, `lagent publish`, registry
- [ ] Documentation — language tour, tutorials, API reference, example agent library

---

## Phase 12 — Token Heap Avancé
Target: evolve the Token Heap from raw segments into a full "context operating system".

### Context Views — Immutable by Default

#### Type `CtxView` — Immutable Reference
- [ ] `CtxView` type — immutable reference to a context segment (or sub-segment)
- [ ] By default, functions receiving a `CtxView` **cannot** call `ctx_append`, `ctx_compress`, or any mutating operation on it — enforced at **compile time**
- [ ] `&CtxView` syntax — explicit borrowing, analogous to Rust references; guarantees non-mutation to callers
- [ ] Use case: pass a sub-part of context to a sub-agent safely, with compile-time immutability guarantees

#### Creation & Slicing
- [ ] `ctx_view(segment)` — creates an immutable view of an entire segment
- [ ] `ctx_view_slice(view, start_token, length)` — creates a sub-view (also immutable)
- [ ] `ctx_view_free(view)` — releases the view handle (not the underlying segment)

#### Inspection & Debugging
- [ ] `inspect(view) -> str` — returns the **exact tokenized representation** that would be sent to the LLM
- [ ] Useful for debugging prompts, logging, and ensuring reproducibility
- [ ] `ctx_len(view) -> u32` — number of tokens in the view
- [ ] `ctx_capacity(view) -> u32` — capacity of the underlying segment

#### Compiler Guarantees
- [ ] Functions can declare parameters as `view: &CtxView` to promise non-mutation
- [ ] Attempting to pass a `&CtxView` to a function expecting `CtxSegment` (mutable) results in a **type error**
- [ ] `infer`, `branch`, `observe` accept `&CtxView` natively
- [ ] Type system enforces: `CtxView` ≠ `CtxSegment` — no implicit coercion

#### VM Opcodes (extensions)
- [ ] `CtxViewCreate` — create immutable view from segment
- [ ] `CtxViewSlice` — create sub-view
- [ ] `CtxViewInspect` — push tokenized string of view onto stack (powers `inspect()`)
- [ ] `CtxViewFree` — release view handle

#### Cohabitation with Existing Features

| Existing Concept | Interaction with `CtxView` | Result |
|------------------|---------------------------|--------|
| `CtxSegment` | Can be converted to `CtxView` via `ctx_view()` | Original segment remains mutable by its owner |
| `ctx_share` | Duplicates a mutable handle, not an immutable view | Distinct; useful for controlled mutable sharing |
| Persistent memory | No impact | Independent |
| Kernel with isolation | Kernel can receive `&CtxView` for `observe` without ability to alter caller | Reinforces isolation promised by kernel |
| `parallel` | Immutable views can be distributed without data race concerns | Secures concurrency |

### Context Guard (CtxGuard) — RAII Pattern

#### Type `CtxGuard` — Automatic Context Restoration
- [ ] `CtxGuard` type — RAII guard for automatic context rollback (Resource Acquisition Is Initialization)
- [ ] `CtxGuard::new(ctx: CtxSegment) -> CtxGuard` — saves current write position of the segment
- [ ] Destructor — automatically restores context to saved position when guard goes out of scope (end of `{ }` block or function)
- [ ] Guarantees rollback even on early return, error, or panic

#### Use Case: Temporary Context Modifications
- [ ] Isolate temporary data (documents to analyze, hypotheses to test) from permanent conversation history
- [ ] Eliminate manual save/restore boilerplate and associated errors
- [ ] Enable clean separation between exploration and commitment phases in agent reasoning

#### Design Principles
- [ ] **Zero new keywords** — only a type `CtxGuard` with its `new()` method
- [ ] **Natural scoping** — uses existing `{ }` blocks to delimit lifetime
- [ ] **Safety** — rollback guaranteed regardless of control flow (return, error, panic)
- [ ] **Composability** — guards can be passed to functions, stored, or nested
- [ ] **Familiarity** — Rust/C++ developers will recognize RAII pattern immediately

#### Example Usage
```la
fn analyser_document(texte: str) -> str {
    let ctx = CtxSegment::new(4096);
    ctx.append("Système : Tu es un assistant concis.");

    let resume = {
        let guard = CtxGuard::new(ctx); // Saves state (empty document context)
        ctx.append("Document à analyser : " + texte);
        infer(ctx)                      // Model sees the document
    }; // <- `guard` destroyed here, document removed from context

    ctx.append("Résumé obtenu : " + resume);
    ctx.append("Maintenant, critique ce résumé.");
    return infer(ctx);                  // Model no longer sees original document
}
```

#### Interaction with Other Phase 12 Features

| Feature | Interaction with `CtxGuard` | Result |
|---------|----------------------------|--------|
| `CtxView` | Guard can protect a segment while immutable views are active | Views remain valid; underlying segment restored |
| `ctx_share` | Guard owns restoration; shared handles see rolled-back state | Predictable semantics |
| Context Versioning | Guard provides lightweight rollback without full versioning | Complementary: versioning for history, guard for scoping |
| Copy-on-Write | Guard restores pre-CoW state | CoW duplications within guard scope are discarded |

### Copy-on-Write (CoW)
- [ ] Replace `Rc<String>` with `Rc<RefCell<...>>` + CoW mechanism in `TokenHeap`
- [ ] `ctx_share` reads shared history without interference; writes (`ctx_append`, `ctx_compress`) trigger lazy duplication
- [ ] Agents can personalise shared context at minimal cost

### Memory-Mapped Context
- [ ] Virtual `CtxSegment` that page-tokens from a memory-mapped file on demand
- [ ] Enable million-token contexts (books, knowledge bases) on modest machines
- [ ] Lazy tokenization from mmap'd source

### Context Swapping
- [ ] `CtxSwapOut(segment) -> SwapId` — offload a context segment to disk or vector store
- [ ] `CtxSwapIn(SwapId) -> segment` — reload a swapped segment back into the Token Heap
- [ ] Manage long conversation histories without saturating RAM

### Context Versioning & Branching
- [ ] Each write operation (`ctx_append`, `ctx_compress`) creates a new version identifier
- [ ] `ctx_revert(version)` — rollback a segment to a previous version
- [ ] Modification journal per segment for "what-if" scenario exploration

### Garbage Collection Sémantique
- [ ] `ctx_alloc_managed()` — auto-managed segment with reference counting (like `Rc`)
- [ ] `ctx_alloc_unmanaged()` — explicit manual control (existing `ctx_alloc` behaviour)
- [ ] Prevent context leaks in complex programs while retaining manual override for critical paths

### Context Diff
- [ ] `ctx.diff(version_a, version_b) -> Diff` — obtain a semantic diff between two versions of a segment
- [ ] Enables `reflect()` and verification loops: agent understands what changed in the context
- [ ] `Diff` type with add/remove/modify entries

### Pagination Explicite
- [ ] `ctx.page_size = 512` — configure page size for massive contexts
- [ ] `ctx.next_page()` / `ctx.prev_page()` — iterate over a massive context without loading it entirely
- [ ] Natural complement to Memory-Mapped Context for very large corpora

### Context Utilities
- [ ] `ctx_merge(seg1, seg2) -> CtxSegment` — combine two segments into one (merge results from parallel agents, avoids manual ctx_append serialization)
- [ ] `ctx_clear(seg)` — empty a segment without deallocating (prevents slab allocator fragmentation, avoids ctx_free + ctx_alloc cycle)
- [ ] `ctx_len(seg) -> u32` — number of tokens used in the segment
- [ ] `ctx_capacity(seg) -> u32` — total capacity of the segment

---

## Phase 13 — Concepts Avancés & Primitives LLM Bas Niveau
Target: advanced reasoning patterns, security capabilities, native vector search, and low-level LLM control.

### Pattern Matching
- [ ] `match` expression for semantic types — `match result { case "success" => ... case "error" => ... case _ => ... }`
- [ ] Union types — `type Response = semantic("success", "error", "pending")` with exhaustive case checking
- [ ] Compile-time warning for non-exhaustive matches on union types
- [ ] Pattern matching on struct fields and enum variants — `match user { case User { age: a } => ... }`

### Capabilities System (Security)
- [ ] `#[capability(network)]`, `#[capability(fs)]` annotations on kernels/functions (uses attribute system from Phase 10)
- [ ] Compiler flags — `--allow-net`, `--allow-fs`, `--allow-all`
- [ ] Compile-time refusal of execution if required capabilities are not granted
- [ ] Strengthens sandboxing for LLM-influenced code paths

### Budget Annotations
- [ ] `#[budget(tokens=5000, latency_ms=2000)]` annotation on functions/kernels (uses attribute system from Phase 10)
- [ ] Compiler warns if estimated token count or latency may exceed declared budget
- [ ] Integrates with `-O cost` / `-O latency` optimisation flags from Phase 9

### Primitives LLM Bas Niveau
- [ ] `logit("token") -> f32` — read raw logit probability of a token before softmax (equivalent of reading a CPU register)
- [ ] `decode with grammar { ... }` — force generation to respect a formal EBNF grammar or JSON schema (constrained decoding)
- [ ] `kv_save(addr)`, `kv_load(addr)`, `kv_drop(prefix)` — explicit save/restore/drop of model KV cache entries (equivalent of `memcpy` on cache)

### Runtime Inference Control
- [ ] `set temperature = 0.7;` — modify inference temperature at runtime (switch between exploration and exploitation)
- [ ] `set seed = 42;` — deterministic seed control for reproducibility
- [ ] `use model "llama3";` — switch the active model within a code block (in-code model switching, distinct from `--backend` CLI flag)

### In-Context Learning
- [ ] `example(input, output)` — define few-shot examples directly in source code; compiler injects them into context
- [ ] Version-controlled few-shot patterns alongside program logic

### Additional Backend Implementations
- [ ] `InferenceBackend` trait already exists → implement concrete backends:
  - [ ] OpenAI Compatible backend (standard API)
  - [ ] Local GGUF backend via `candle` or `llama-cpp-rs`
  - [ ] ONNX backend via `ort` (ONNX Runtime)
- [ ] User-selectable at compile-time or runtime via `--backend`

### Advanced Structured Reasoning
- [ ] `decompose(problem) -> [SubProblem]` — automatically split a problem into sub-tasks
- [ ] `reflect()` — agent analyses its own execution trace to self-improve
- [ ] `critique(other_agent_output)` — one agent evaluates another's output
- [ ] Extend beyond `observe`, `reason`, `act`, `verify` primitives

### Native Embedding & Vector Search
- [ ] `embed(text: str) -> Vec<f32>` — built-in embedding primitive
- [ ] `store_embedding(id, embedding)` — persist embeddings for later retrieval
- [ ] `search_similar(embedding, k) -> [id]` — k-nearest-neighbour search in embedding space
- [ ] `cosine_sim(a: Vec<f32>, b: Vec<f32>) -> f32` — native cosine similarity computation (equivalent of a SIMD instruction)
- [ ] Enable long-term memory and RAG patterns natively in the language

### Méta-cognition
- [ ] `uncertainty() -> f32` — returns the agent's global confidence level in its last inference or action
- [ ] Enables the agent to self-trigger fallbacks or clarification requests instead of relying solely on static thresholds
- [ ] Dynamic alternative to fixed confidence thresholds in `branch`

---

## Milestones

| Milestone | Description                                                | Status    |
|-----------|------------------------------------------------------------|-----------|
| v0.1      | Phase 1 — Proof of Concept                                 | ✅ Complete |
| v0.2      | Phase 2 — Semantic Types                                   | ✅ Complete |
| v0.3      | Phase 3 — Kernel Frames                                    | ✅ Complete |
| v0.4      | Phase 4 — Agent Vocabulary                                 | ✅ Complete |
| v0.5      | Phase 5 — Sémantique Stricte des Primitives Agentiques     | Planned   |
| v0.6      | Phase 6 — Constraint Enforcement & Visibility              | Planned |
| v0.7      | Phase 7 — Fondamentaux du Langage                          | Planned   |
| v0.8      | Phase 8 — Robustesse & Expressivité                        | Planned   |
| v0.9      | Phase 9 — Parallélisme & Ressources                        | Planned   |
| v0.10     | Phase 10 — Méta-programmation & Génériques                 | Planned   |
| v0.11     | Phase 11 — Interopérabilité & Écosystème                   | Planned   |
| v0.12     | Phase 12 — Token Heap Avancé                               | Planned   |
| v0.13     | Phase 13 — Concepts Avancés & Primitives LLM Bas Niveau    | Planned   |
