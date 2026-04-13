# L-Agent

> A systems programming language for the LLM era.

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-pre--alpha-yellow.svg)](docs/ROADMAP.md)

**C gave you control over silicon. L-Agent gives you control over language models.**

L-Agent (`lagent`, files: `.la`) is a **compiled, statically-typed language** that gives programmers explicit, low-level control over LLM inference primitives — context windows, probabilistic branching, reasoning kernels, and agent identity — in the same way C gives control over hardware registers and memory.

---

## Why L-Agent?

Today, programming agents means gluing Python scripts together, hoping prompt strings work, and having **zero guarantees** about what the LLM will do. There's no type system, no compiler, no resource control.

L-Agent flips the model:

| What you get | What it replaces |
|---|---|
| `type Mood = semantic("happy", "sad", "neutral")` constrains LLM output at compile time | Hoping the model "responds correctly" |
| `kernel { observe; reason; act; verify }` with auto-retry on failure | Prompt + pray |
| `ctx_alloc(4096)` / `ctx_free(ctx)` for explicit token budgeting | Blind context window exhaustion |
| `branch intent { case "angry" (confidence > 0.7) => ... }` for probabilistic control flow | if/else on string outputs |
| Compile-time `fn`/`skill`/`kernel` safety guarantees | Everything is `def` and hope |

---

## Quick Example

```la
// Define a type that constrains the LLM's output to one of these labels.
type Mood = semantic("happy", "sad", "neutral");

// The agent's identity — injected as system preamble before execution.
soul {
    instruction "You are a helpful sentiment analysis agent.";
    instruction "Always respond concisely.";
}

// Static knowledge injected into the agent's system context.
lore Background = "This agent analyses user-provided text for emotional tone.";

// Named persistent slot — survives kernel resets within a run.
memory LastResult: str = "";

// External capability — resolved by the backend at runtime.
oracle FetchContext(url: str) -> str;

// Reusable guard — inlined at call site, non-retriable if violated.
constraint NonEmpty {
    verify(result != "");
}

// A safe, composable agent capability.
pub skill AnalyseMood(text: str) -> Mood {
    observe(text);
    reason("Classify the mood of the text");
    let result: Mood = infer(text);
    apply NonEmpty;
    return result;
}

fn main() {
    let ctx = ctx_alloc(1024);
    ctx_append(ctx, "I love this project, it is amazing!");
    let mood = AnalyseMood(ctx);
    println(mood);
    ctx_free(ctx);
}
```

---

## Core Concepts: The Four Primitives

L-Agent distinguishes four levels of computation, each with **compile-time guarantees**:

| Primitive | LLM Access? | Context Access? | Auto-Retry? | Use Case |
|-----------|:-----------:|:---------------:|:-----------:|----------|
| **`fn`** | ❌ | ❌ | — | Pure computation — algorithms, math, data transforms |
| **`skill`** | ❌ (but `#[tool]` exposable) | ❌ | — | Deterministic capability — safe for LLM function calling |
| **`spell`** | ✅ | ✅ | — | Orchestrated workflow — flexible, no imposed structure |
| **`kernel`** | ✅ | ✅ (isolated) | ✅ | Transactional reasoning — observe → reason → act → verify with rollback |

```la
// fn: pure, no LLM, no context — like a C function
fn add_tax(price: f32, rate: f32) -> f32 {
    return price * (1.0 + rate);
}

// skill: agent capability, can be #[tool] — deterministic & safe
#[tool]
skill classify_ticket(text: str) -> Category {
    let cat = infer(text);
    return cat;
}

// spell: flexible workflow — all primitives allowed
spell handle_complaint(client: str, msg: str) -> Response {
    let ctx = ctx_alloc(2048);
    ctx_append(ctx, msg);
    branch intent {
        case "refund" (confidence > 0.7) => { /* ... */ }
        default => { /* ... */ }
    }
    ctx_free(ctx);
}

// kernel: transactional reasoning with isolation & auto-retry
kernel verify_diagnosis(symptoms: str) -> Diagnosis {
    observe(symptoms);
    reason("List possible causes ranked by probability");
    act {
        let diag = infer<Diagnosis>();
        return diag;
    }
    verify(|d| d.confidence > 0.95);
    // If verify fails → full rollback & re-execution from observe (up to 3×)
}
```

---

## Key Features

### Language

| Feature | Description |
|---------|-------------|
| **Semantic types** | `type Mood = semantic("happy", "sad", ...)` — output constrained via decoding |
| **Probabilistic branching** | `branch intent { case "angry" (confidence > 0.7) => ... }` |
| **Reasoning kernels** | `kernel K() { observe; reason; act; verify; }` — traceable, retriable, isolated |
| **Agent vocabulary** | `soul`, `skill`, `spell`, `memory`, `oracle`, `constraint`, `lore` — declarative identity |
| **Constraint enforcement** | `apply ConstraintName;` — inline guard blocks, non-retriable `ConstraintViolation` |
| **Token heap management** | `ctx_alloc` / `ctx_free` / `ctx_compress` — explicit context budgeting |
| **Persistent memory** | `memory` (intra-run) and `persistent memory` (inter-run via `--persist`) |
| **Module system** | `use "module.la";` — inline import expansion, `pub` visibility |
| **Library bundles** | `lagent build --lib` → `.lalb` precompiled bundle with export table |
| **Auto-formatter** | `lagent fmt` — normalised 4-space-indented source |

### Runtime

| Feature | Description |
|---------|-------------|
| **Multiple backends** | Simulated (default) or Anthropic API (`--backend anthropic`) |
| **Deterministic mode** | `--deterministic` — temperature=0 for reproducible inference |
| **Bytecode execution** | `.lbc` compiled output executed on a stack-based VM |

### Tooling

| Feature | Description |
|---------|-------------|
| **`lagent build`** | Compile `.la` → `.lbc` bytecode |
| **`lagent run`** | Compile + execute (simulated or remote backend) |
| **`lagent check`** | Syntax & semantic analysis without codegen |
| **`lagent fmt`** | Auto-format source files in place |
| **`lagent.toml`** | Project manifest — entry point, models, optimization strategy |

---

## Installation

**Prerequisites:** Rust 1.78+ ([install via rustup](https://www.rust-lang.org/tools/install))

```bash
cargo install --path lagent-cli
```

For the Anthropic backend (remote API):

```bash
cargo install --path lagent-cli --features backend-remote
```

---

## Usage

```bash
# Compile
lagent build src/main.la                         # → main.lbc
lagent build --lib src/lib.la                    # → lib.lalb (library bundle)

# Execute
lagent run   src/main.la                         # compile + run (simulated)
lagent run   --backend anthropic src/main.la     # compile + run (Anthropic API)
lagent run   --deterministic src/main.la         # temperature=0
lagent run   --persist store.json src/main.la    # attach cross-run persistence

# Tooling
lagent check src/main.la                         # syntax/semantic check only
lagent fmt   src/main.la                         # auto-format in place
lagent fmt   --check src/main.la                 # exit non-zero if file would change
```

### Project Manifest

When `lagent.toml` is present, `input` is optional:

```bash
lagent build          # uses project.entry from lagent.toml
lagent build --lib    # uses lib.entry and lib.name from lagent.toml
```

Example `lagent.toml`:

```toml
[project]
name = "my-agent"
version = "0.1.0"
entry = "src/main.la"

[models]
default = "simulated"

[compile]
optimization = "cost"   # cost | precision | latency | local
context_limit = 8192
```

### Remote Backend

```bash
LAGENT_API_KEY=sk-... lagent run --backend anthropic examples/agent_soul.la
```

---

## Project Structure

```
lagent-compiler/         # .la → bytecode compiler
├── src/
│   ├── lexer/           # logos-based tokeniser
│   ├── parser/          # chumsky parser → AST
│   ├── codegen/         # 3-pass bytecode generator + opcodes
│   ├── semantic/        # name resolution, type checking
│   ├── resolver.rs      # module import expansion (pub visibility)
│   ├── project.rs       # lagent.toml manifest
│   └── fmt.rs           # AST pretty-printer (lagent fmt)

lagent-vm/               # bytecode executor
├── src/
│   ├── vm.rs            # stack-based VM, opcode dispatch
│   ├── backends/        # InferenceBackend trait, simulated, anthropic
│   ├── persistent_store.rs  # PersistentStore trait + FilePersistentStore
│   └── runtime/         # TokenHeap (context segment allocator)

lagent-cli/              # lagent build/run/check/fmt binary
examples/                # example .la programs
docs/                    # SPEC.md, ARCHITECTURE.md, ROADMAP.md
```

---

## Development Status

**Pre-alpha — Phase 7 en cours.** Le compilateur complet (pipeline + VM + toolchain) est fonctionnel. Les fondamentaux du langage (contrôle de flux, struct/enum, Vec, const, mut, type safety) sont implémentés. Persistence unification, bytecode cleanup et error handling restent à faire.

| Phase | Description | Status |
|-------|-------------|:------:|
| 1 | Proof of concept — lexer, parser, VM | ✅ |
| 2 | Semantic types, kernels, probabilistic branching | ✅ |
| 3 | Kernel call frames, verify retry, interruptible blocks | ✅ |
| 4 | Agent vocabulary, module system, remote backend | ✅ |
| 5 | Sémantique stricte — `fn`, `skill`, `spell`, `kernel` | ⏳ |
| 6 | Constraint enforcement, `pub`, persistent memory, `.lalb`, `fmt` | ⏳ |
| 7 | Fondamentaux — loops, if/else, struct/enum, Vec, `const`, `mut`, type safety | 🔄 |
| 8 | Robustesse — `Dist<T>`, `AgentError`, fallbacks, token tracking | ⏳ |
| 9 | Parallélisme — `parallel`/`race`, merge strategies, `-O` flags | ⏳ |
| 10 | Méta-programmation — macros, attributes, lambdas, generics | ⏳ |
| 11 | Interopérabilité — FFI, MCP, `pub(crate)`, NERD, package manager | ⏳ |
| 12 | Token Heap avancé — Views, CoW, mmap, swapping, versioning | ⏳ |
| 13 | Concepts avancés — pattern matching, capabilities, logit, KV cache | ⏳ |

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the full roadmap.

---

## Documentation

| Document | Description |
|----------|-------------|
| [Language Specification](docs/SPEC.md) | Complete syntax and semantics reference |
| [Architecture](docs/ARCHITECTURE.md) | Compiler pipeline, VM design, crate structure |
| [Roadmap](docs/ROADMAP.md) | Development phases, planned features, milestones |

---

## Contributing

PRs welcome! See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full guide.

**Quick start:**

```bash
git clone https://github.com/lagent-lang/lagent
cd lagent
cargo build
cargo test
```

**Before submitting a PR, run:**

```bash
cargo fmt --all && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace --all-features
```

---

## License

Apache-2.0 — see [`LICENSE`](LICENSE).
