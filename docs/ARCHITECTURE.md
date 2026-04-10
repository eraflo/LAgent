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
|  parser (chumsky)  |    |   +- InferenceBackend  |
|       |            |    |   |    +- Simulated    |
|  semantic          |    |   |    +- Remote API   |
|       |            |    |   |    +- Local GGUF   |
|  codegen           |    |   +- OpCode dispatcher |
|  -> .lbc bytecode  |    |                        |
+--------------------+    +------------------------+
```

## Crate Responsibilities

### `lagent-compiler`
Transforms `.la` source code into `.lbc` bytecode in four passes:

1. **Lexer** (`src/lexer/mod.rs`): tokenizes source using `logos`.
2. **Parser** (`src/parser/`): builds an untyped AST using `chumsky` parser combinators.
3. **Semantic analysis** (`src/semantic/mod.rs`): resolves names, checks types, validates semantic constraints.
4. **Code generation** (`src/codegen/`): emits `OpCode` instructions serialized via `bincode`.

### `lagent-vm`
Executes bytecode. Key components:

- **`Vm`** (`src/vm.rs`): main execution loop, dispatches opcodes.
- **`TokenHeap`** (`src/runtime/token_heap.rs`): manages context segments. O(n) alloc, O(1) free by id.
- **`InferenceBackend`** trait (`src/backends/mod.rs`): abstracts model calls. Feature-flagged backends:
  - `backend-simulated` (default): deterministic, no model required.
  - `backend-remote`: HTTP calls via `reqwest` to OpenAI/Anthropic APIs.
  - `backend-local-gguf`: local model via `llama-cpp-bindings`.

### `lagent-cli`
Thin binary wrapping compiler + VM. Commands: `build`, `run`, `check`.

## Data Flow: `lagent run hello.la`

```
hello.la
   |  read_to_string
   v
lagent_compiler::compile(source)
   |  lexer::tokenize  ->  Vec<Token>
   |  parser::parse    ->  Vec<Item>  (AST)
   |  semantic::analyze ->  TypedAst
   |  codegen::generate ->  Vec<u8>  (.lbc bytes)
   v
lagent_vm::Vm::execute(bytecode)
   |  deserialize -> Bytecode { instructions: Vec<OpCode> }
   |  loop: dispatch each OpCode
   |    CtxAlloc   -> TokenHeap::alloc
   |    Branch     -> InferenceBackend::classify
   |    CallKernel -> kernel execution loop
   v
  stdout / side effects
```

## Extending the Compiler

To add a new keyword (e.g., `interruptible`):
1. Add `#[token("interruptible")] Interruptible` to `lexer/mod.rs`.
2. Add the corresponding AST node in `parser/ast.rs`.
3. Handle it in the `chumsky` parser in `parser/mod.rs`.
4. Add semantic validation in `semantic/mod.rs`.
5. Emit the appropriate `OpCode` in `codegen/mod.rs`.
6. Implement the opcode in `vm.rs`.

## Extending Backends

Implement `InferenceBackend` for your backend, gate it behind a Cargo feature flag, and register it in `backends/mod.rs`.
