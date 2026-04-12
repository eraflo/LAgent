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

skill AnalyseMood(text: str) -> Mood {
    observe(text);
    reason("Classify the mood of the text");
    let result: Mood = infer(text);
    verify(result != "");
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

## Key Features

| Feature | Description |
|---------|-------------|
| **Semantic types** | `type Mood = semantic("happy", "sad", ...)` — runtime-classified via constrained decoding |
| **Probabilistic branching** | `branch intent { case "angry" (confidence > 0.7) => ... }` |
| **Reasoning kernels** | `kernel K() { observe; reason; act; verify; }` — traceable, retriable units |
| **Agent vocabulary** | `soul`, `skill`, `spell`, `memory`, `oracle`, `constraint`, `lore` — declarative agent identity |
| **Module system** | `use "module.la";` — inline import expansion at compile time |
| **Context heap** | `ctx_alloc / ctx_free` — explicit token budget management |
| **Multiple backends** | Simulated (default) or Anthropic API (`--backend anthropic`) |

## Installation

```bash
cargo install --path lagent-cli
```

## Usage

```bash
lagent build src/main.la                         # compile to main.lbc
lagent run   src/main.la                         # compile + execute (simulated backend)
lagent run   --backend anthropic src/main.la     # compile + execute (Anthropic API)
lagent run   --deterministic src/main.la         # temperature=0 inference
lagent check src/main.la                         # syntax/type check only
```

For the Anthropic backend, set `LAGENT_API_KEY` in your environment. Recompile with `--features backend-remote` to enable it:

```bash
cargo install --path lagent-cli --features backend-remote
LAGENT_API_KEY=sk-... lagent run --backend anthropic examples/agent_soul.la
```

## Project Layout

```
lagent-compiler/   # .la → .lbc compiler (lexer, parser, semantic, codegen)
lagent-vm/         # bytecode executor (VM, TokenHeap, InferenceBackend)
lagent-cli/        # lagent build/run/check binary
examples/          # example .la programs
docs/              # SPEC.md, ARCHITECTURE.md, ROADMAP.md
```

## Status

**Pre-alpha — Phase 4 complete.** The full compiler pipeline and VM are functional. The agent vocabulary keywords (`soul`, `skill`, `spell`, `memory`, `oracle`, `constraint`, `lore`) are parsed, compiled, and executed. A module system (`use "path.la"`) and remote Anthropic backend are available.

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the full development roadmap.

## Architecture

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Language Reference

See [`docs/SPEC.md`](docs/SPEC.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Apache-2.0
