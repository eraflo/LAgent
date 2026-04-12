# L-Agent Architecture

## Overview

```
+----------------------------------------------------------+
|                    lagent CLI                            |
|              (lagent-cli/src/main.rs)                    |
+------------------------+---------------------------------+
                         |
            +------------+------------+
            |                         |
            v                         v
+--------------------+    +------------------------+
| lagent-compiler    |    |     lagent-vm          |
|                    |    |                        |
|  lexer (logos)     |    |  Vm                    |
|       |            |    |   +- TokenHeap         |
|  parser (chumsky)  |    |   +- soul_meta         |
|       |            |    |   +- memory (slots)    |
|  resolver          |    |   +- lore (table)      |
|       |            |    |   +- InferenceBackend  |
|  semantic          |    |   |    +- Simulated    |
|       |            |    |   |    +- Anthropic *  |
|  codegen           |    |   +- OpCode dispatcher |
|  -> .lbc bytecode  |    |                        |
+--------------------+    +------------------------+

* feature-gated: backend-remote
```

## Crate Responsibilities

### `lagent-compiler`
Transforms `.la` source code into `.lbc` bytecode in four stages:

1. **Lexer** (`src/lexer/mod.rs`): tokenizes source using `logos`. Produces `Vec<Token>`.
2. **Parser** (`src/parser/`): builds an AST using `chumsky` parser combinators. Produces `Vec<Item>`.
3. **Resolver** (`src/resolver.rs`): expands `use "path.la"` imports inline (recursive, depth-first). Produces a flat `Vec<Item>` with all imported items prepended.
4. **Semantic analysis** (`src/semantic/mod.rs`): resolves names, validates identifiers, builds `TypedAst` (type env, oracle names, lore table).
5. **Code generation** (`src/codegen/`): emits `OpCode` sequences in three passes, serialized via `bincode`.

#### Codegen — Three Passes

| Pass | Items processed | Output |
|------|-----------------|--------|
| 0 — Lore | `LoreDecl` | `StoreLore` opcodes prepended to main stream |
| 1 — Kernels | `KernelDef`, `SpellDef`, `SkillDef` | `KernelBytecode` table (callable via `CallKernel`) |
| 2 — Main | `FnDef`, `MemoryDecl` | Flat instruction stream (soul preamble before `fn main`) |

Skills are compiled into the kernel table (same as spells), making them callable via `CallKernel`.
The soul preamble (`SetAgentMeta` + `CtxAppendLiteral` instructions) is injected before the body of `fn main`.

#### Entry Points

- `compile(src: &str) -> Result<Vec<u8>>` — in-memory pipeline, no filesystem access.
- `compile_file(path: &Path) -> Result<Vec<u8>>` — reads source, runs `resolve_uses` for module imports, then full pipeline.

### `lagent-vm`
Executes bytecode. Key components:

- **`Vm`** (`src/vm.rs`): main execution loop, dispatches opcodes. Carries:
  - `heap: TokenHeap` — context segment allocator.
  - `backend: Box<dyn InferenceBackend>` — inference abstraction.
  - `soul_meta: Option<String>` — agent identity string (set by `SetAgentMeta`).
  - `memory: HashMap<String, Value>` — named persistent slots (survive frame resets).
  - `lore: HashMap<String, String>` — static knowledge strings.
- **`TokenHeap`** (`src/runtime/token_heap.rs`): slab allocator for context segments. O(n) alloc, O(1) free by id.
- **`InferenceBackend`** trait (`src/backends/mod.rs`): required methods:
  - `infer(prompt) -> String`
  - `classify(prompt, labels) -> String`
  - `compress(text) -> String`
  - `act(payload) -> String`
  - `oracle(name, args) -> String`
  - Feature-flagged implementations:
    - `SimulatedBackend` (default): deterministic, no model required.
    - `AnthropicBackend` (`backend-remote`): HTTP via `reqwest` to Anthropic Messages API.

### `lagent-cli`
Thin binary wrapping compiler + VM. Commands: `build`, `run`, `check`.

Runtime flags for `run`:
- `--backend simulated|anthropic` (default: `simulated`)
- `--deterministic` — passes temperature=0 to backend
- `--context N` — token heap size (default: 4096)

## Data Flow: `lagent run agent_soul.la`

```
agent_soul.la
   |  read_to_string
   v
lagent_compiler::compile_file(path)
   |  lexer::tokenize    ->  Vec<Token>
   |  parser::parse      ->  Vec<Item>  (AST)
   |  resolver::resolve_uses  ->  Vec<Item>  (imports inlined)
   |  semantic::analyze  ->  TypedAst { items, type_env, oracle_names, lore_table }
   |  codegen::generate  ->  Vec<u8>  (.lbc bytes)
   |    Pass 0: StoreLore opcodes
   |    Pass 1: KernelBytecode table (kernels + spells + skills)
   |    Pass 2: main stream (soul preamble + fn main body)
   v
lagent_vm::Vm::execute(bytecode)
   |  deserialize -> Bytecode { kernels, instructions }
   |  loop: dispatch each OpCode
   |    StoreLore       -> vm.lore.insert(...)
   |    AllocMemorySlot -> vm.memory.insert(...)
   |    SetAgentMeta    -> vm.soul_meta = Some(...)
   |    CtxAlloc        -> TokenHeap::alloc
   |    CallKernel      -> push kernel frame, execute body, pop frame
   |    BranchClassify  -> InferenceBackend::classify
   |    CallOracle      -> InferenceBackend::oracle
   v
  stdout / side effects
```

## Extending the Compiler

To add a new keyword:
1. Add `#[token("keyword")] Keyword` to `lexer/mod.rs`.
2. Add the corresponding AST node in `parser/ast.rs`.
3. Add a parser combinator in `parser/mod.rs` and wire it into `program()`.
4. Add semantic validation in `semantic/mod.rs` (register names, check bodies).
5. Emit the appropriate `OpCode`(s) in `codegen/mod.rs`.
6. Implement the opcode arm(s) in `vm.rs`.

## Extending Backends

Implement `InferenceBackend` (all five methods) for your backend, gate it behind a Cargo feature flag, and add a conditional re-export in `backends/mod.rs`.
