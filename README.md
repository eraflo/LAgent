# L-Agent

> A systems programming language for the LLM era.

L-Agent (`lagent`, files: `.la`) is a compiled, statically-typed language that gives programmers explicit, low-level control over LLM inference primitives — context windows, probabilistic branching, reasoning kernels, and agent identity — in the same way C gives control over hardware registers and memory.

## Quick Example

```la
type Mood = semantic("happy", "sad", "neutral");

soul {
    instruction "You are a helpful sentiment analysis agent.";
}

lore Background = "This agent analyses user-provided text for emotional tone.";

memory LastResult: str = "";

oracle FetchContext(url: str) -> str;

constraint NonEmpty {
    verify(result != "");
}

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
    memory_save("last_mood", mood);
    println(mood);
    ctx_free(ctx);
}
```

## Key Features

| Feature | Description |
|---------|-------------|
| **Semantic types** | `type Mood = semantic("happy", "sad", ...)` — runtime-classified via constrained decoding |
| **Probabilistic branching** | `branch intent { case "angry" (confidence > 0.7) => ... }` |
| **Reasoning kernels** | `kernel K() { observe; reason; act; verify; }` — traceable, retriable units |
| **Agent vocabulary** | `soul`, `skill`, `spell`, `memory`, `oracle`, `constraint`, `lore` — declarative agent identity |
| **Constraint enforcement** | `apply ConstraintName;` — inline guard blocks, non-retriable `ConstraintViolation` |
| **Module system** | `use "module.la";` — inline import expansion, `pub` visibility across modules |
| **Library bundles** | `lagent build --lib` → `.lalb` precompiled bundle with export table |
| **Persistent memory** | `memory_save/load/delete` — cross-run key-value store (`--persist store.json`) |
| **Context heap** | `ctx_alloc / ctx_free` — explicit token budget management |
| **Multiple backends** | Simulated (default) or Anthropic API (`--backend anthropic`) |
| **Auto-formatter** | `lagent fmt` — normalised 4-space-indented source |

## Installation

```bash
cargo install --path lagent-cli
```

## Usage

```bash
lagent build src/main.la                         # compile to main.lbc
lagent build --lib src/lib.la                    # compile to lib.lalb (library bundle)
lagent run   src/main.la                         # compile + execute (simulated backend)
lagent run   --backend anthropic src/main.la     # compile + execute (Anthropic API)
lagent run   --deterministic src/main.la         # temperature=0 inference
lagent run   --persist store.json src/main.la    # attach cross-run persistent store
lagent check src/main.la                         # syntax/semantic check only
lagent fmt   src/main.la                         # auto-format in place
lagent fmt   --check src/main.la                 # exit non-zero if file would change
```

When `lagent.toml` is present at the project root, the `input` argument is optional:

```bash
lagent build          # uses project.entry from lagent.toml
lagent build --lib    # uses lib.entry and lib.name from lagent.toml
```

For the Anthropic backend, set `LAGENT_API_KEY` and recompile with `--features backend-remote`:

```bash
cargo install --path lagent-cli --features backend-remote
LAGENT_API_KEY=sk-... lagent run --backend anthropic examples/agent_soul.la
```

## Project Layout

```
lagent-compiler/   # .la → bytecode compiler
  src/
    lexer/         # logos-based tokeniser
    parser/        # chumsky parser → AST
    codegen/       # 3-pass bytecode generator + opcodes
    semantic/      # name resolution, type checking
    resolver.rs    # module import expansion (pub visibility)
    project.rs     # lagent.toml manifest
    fmt.rs         # AST pretty-printer (lagent fmt)

lagent-vm/         # bytecode executor
  src/
    vm.rs          # stack-based VM, opcode dispatch
    backends/      # InferenceBackend trait, simulated, anthropic
    persistent_store.rs  # PersistentStore trait + FilePersistentStore
    runtime/       # TokenHeap (context segment allocator)

lagent-cli/        # lagent build/run/check/fmt binary
examples/          # example .la programs
docs/              # SPEC.md, ARCHITECTURE.md, ROADMAP.md
```

## Status

**Pre-alpha — Phase 5 complete.** The full compiler pipeline, VM, and toolchain are functional.

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Proof of concept — lexer, parser, VM | ✅ |
| 2 | Semantic types, kernels, probabilistic branching | ✅ |
| 3 | Kernel call frames, verify retry, interruptible blocks | ✅ |
| 4 | Agent vocabulary, module system, remote backend | ✅ |
| 5 | Constraint enforcement, `pub` visibility, persistent memory, `.lalb`, `lagent fmt` | ✅ |
| 6 | Production tooling — LSP, debugger, optimisations, FFI | Planned |

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the full development roadmap.

## Architecture

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Language Reference

See [`docs/SPEC.md`](docs/SPEC.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Apache-2.0
