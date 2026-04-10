# L-Agent

> A systems programming language for the LLM era.

L-Agent (`lagent`, files: `.la`) is a compiled, statically-typed language that gives programmers explicit, low-level control over LLM inference primitives — context windows, probabilistic branching, and reasoning kernels — in the same way C gives control over hardware registers and memory.

## Quick Example

```la
type Emotion = semantic("joie", "colère", "tristesse", "neutre");

kernel AnalyserMessage(texte: str) -> Emotion {
    observe(texte);
    reason("Déterminer l'émotion dominante");
    let emotion: Emotion = infer(texte);
    verify(emotion != "neutre");
    return emotion;
}

fn main() {
    let ctx = ctx_alloc(4096);
    ctx_append(ctx, "Je suis très mécontent de ce service !");

    branch intent {
        case "angry" (confidence > 0.7) => {
            println("Gestion de crise activée");
        }
        default => {
            println("Redirection vers un opérateur humain");
        }
    }

    ctx_free(ctx);
}
```

## Key Features

| Feature | Description |
|---------|-------------|
| **Semantic types** | `type Emotion = semantic("joie", "colère", ...)` — validated at runtime via embedding distance |
| **Probabilistic branching** | `branch intent { case "angry" (confidence > 0.7) => ... }` — constrained decoding |
| **Reasoning kernels** | `kernel K() { observe; reason; act; verify; }` — traceable, retriable units |
| **Context heap** | `ctx_alloc / ctx_free` — explicit token budget management |
| **Multiple backends** | Simulated, local GGUF, remote API — switchable at compile or runtime |

## Installation

```bash
cargo install --path lagent-cli
```

## Usage

```bash
lagent build src/main.la          # compile to main.lbc
lagent run   src/main.la          # compile + execute
lagent check src/main.la          # syntax/type check only
```

## Status

**Pre-alpha.** Currently in Phase 1 (see `docs/ROADMAP.md`). The compiler pipeline skeleton is in place; full parser and VM are under active development.

## Architecture

See `docs/ARCHITECTURE.md`.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT OR Apache-2.0
