# L-Agent Language Specification

**Version 0.5 — April 2026**

## 1. Lexical Grammar

### 1.1 Keywords
```
fn kernel branch case default type let return pub use interruptible apply
observe reason act verify infer
ctx_alloc ctx_free ctx_append ctx_resize ctx_compress ctx_share
memory_load memory_save memory_delete
println semantic intent
str bool u32 f32
soul skill instruction spell memory oracle constraint lore
```

### 1.2 Literals
- String literals: `"..."` with standard escape sequences
- Integer literals: `[0-9]+`
- Float literals: `[0-9]+\.[0-9]+`

### 1.3 Identifiers
`[a-zA-Z_][a-zA-Z0-9_]*`

### 1.4 Comments
Line comments: `// ...`

---

## 2. Grammar (EBNF)

```ebnf
program     = item* ;

item        = fn_def
            | kernel_def
            | type_alias
            | soul_def
            | skill_def
            | spell_def
            | memory_decl
            | oracle_decl
            | constraint_def
            | lore_decl
            | use_decl ;

fn_def          = "fn" IDENT "(" params ")" ("->" type_expr)? block ;
kernel_def      = "kernel" IDENT "(" params ")" "->" type_expr block ;
type_alias      = "type" IDENT "=" type_expr ";" ;
soul_def        = "soul" block ;
skill_def       = "pub"? "skill" IDENT "(" params ")" ("->" type_expr)? block ;
spell_def       = "spell" IDENT "(" params ")" "->" type_expr block ;
memory_decl     = "memory" IDENT ":" type_expr "=" expr ";" ;
oracle_decl     = "oracle" IDENT "(" params ")" "->" type_expr ";" ;
constraint_def  = "constraint" IDENT block ;
lore_decl       = "lore" IDENT "=" STRING ";" ;
use_decl        = "use" STRING ";" ;

params      = (param ("," param)*)? ;
param       = IDENT ":" type_expr ;

type_expr   = "semantic" "(" STRING ("," STRING)* ")"
            | IDENT
            | prim_type ;

prim_type   = "str" | "bool" | "u32" | "f32" ;

block       = "{" stmt* "}" ;

stmt        = let_stmt
            | return_stmt
            | branch_stmt
            | interruptible_stmt
            | instruction_stmt
            | apply_stmt
            | expr_stmt ;

let_stmt            = "let" IDENT (":" type_expr)? "=" expr ";" ;
return_stmt         = "return" expr ";" ;
branch_stmt         = "branch" IDENT "{" branch_case* ("default" "=>" block)? "}" ;
branch_case         = "case" STRING "(" "confidence" ">" FLOAT ")" "=>" block ;
interruptible_stmt  = "interruptible" block ;
instruction_stmt    = "instruction" STRING ";" ;
apply_stmt          = "apply" IDENT ";" ;
expr_stmt           = expr ";" ;

expr        = call_expr | IDENT | STRING | INT | FLOAT | bin_expr ;
call_expr   = IDENT "(" (expr ("," expr)*)? ")" ;
bin_expr    = expr bin_op expr ;
bin_op      = "!=" | ">" | "<" ;
```

---

## 3. Type System

### 3.1 Primitive Types
| Type   | Description            |
|--------|------------------------|
| `str`  | UTF-8 string           |
| `bool` | Boolean                |
| `u32`  | 32-bit unsigned int    |
| `f32`  | 32-bit float           |

### 3.2 Semantic Types
```la
type Name = semantic("concept1", "concept2", ...);
```

A semantic type defines a named set of concepts. At runtime, `infer(expr)` with a semantic return type emits `InferClassify(labels)`, which asks the backend to constrained-decode among the declared labels.

### 3.3 Context Segments
`CtxSegment` is a first-class resource type returned by `ctx_alloc`. It represents a named window into the LLM's context. Must be explicitly freed with `ctx_free`.

---

## 4. Context Primitives

| Primitive                               | Description                                  |
|-----------------------------------------|----------------------------------------------|
| `ctx_alloc(tokens: u32) -> CtxSegment`  | Allocate a context segment                   |
| `ctx_free(seg: CtxSegment)`             | Free a context segment                       |
| `ctx_append(seg: CtxSegment, s: str)`   | Append text to a segment                     |
| `ctx_resize(seg: CtxSegment, n: u32)`   | Resize a segment                             |
| `ctx_compress(seg: CtxSegment)`         | Summarize segment content to reclaim tokens  |
| `ctx_share(seg: CtxSegment)`            | Duplicate a context handle reference         |

---

## 5. Branch Statement

```la
branch <var> {
    case "label" (confidence > threshold) => { ... }
    default => { ... }
}
```

Semantics:
1. The runtime infers the probability distribution over case labels using constrained decoding.
2. Cases are evaluated in order. The first case whose confidence exceeds the threshold is executed.
3. If no case matches, the `default` block runs.
4. If `default` is absent and no case matches, execution continues silently.

---

## 6. Kernel Blocks

```la
kernel Name(params) -> ReturnType {
    observe(expr);
    reason("instruction");
    act(expr);
    verify(condition);
    return value;
}
```

Each step is traced. If `verify` fails, the kernel retries up to `MAX_KERNEL_RETRIES` times (default: 3) before propagating a `KernelVerifyError`.

Kernels, spells, and skills are all compiled into the kernel table and callable via the same dispatch mechanism.

---

## 7. Interruptible Blocks

```la
interruptible {
    // ... statements that may be interrupted
}
```

A safe interaction point. On entry, a checkpoint of the current frame is saved. If an error occurs inside the block, the frame is restored to the checkpoint and execution resumes after the block.

---

## 8. Agent Vocabulary

These keywords form the **high-level vocabulary** of L-Agent. They describe agent identity, capabilities, and knowledge declaratively, complementing the imperative primitives (`kernel`, `branch`, `ctx_*`).

---

### 8.1 `soul` — Agent Identity

Defines the agent's persistent identity. Instructions inside a `soul` block are emitted as a preamble before `fn main` executes, appending to any in-scope context handle.

```la
soul {
    instruction "You are a helpful sentiment analysis agent.";
    instruction "Always respond concisely.";
}
```

At the bytecode level, `soul` emits `SetAgentMeta("soul")` followed by `CtxAppendLiteral` for each `instruction` statement.

---

### 8.2 `skill` — Agent Capability

Declares a callable agent capability. A skill has the same syntax as a function with an explicit parameter list and optional return type. Skills are compiled into the kernel table and callable via `CallKernel`.

```la
skill AnalyseMood(text: str) -> Mood {
    observe(text);
    reason("Classify the mood of the text");
    let result: Mood = infer(text);
    verify(result != "");
    return result;
}
```

Skills can use `observe`, `reason`, `verify`, `infer`, and all other kernel primitives.

---

### 8.3 `instruction` — System Directive

Inside a `soul` or `skill` body, appends a literal string to the active context handle. Emits `CtxAppendLiteral(text)`.

```la
soul {
    instruction "You are a helpful assistant.";
}
```

---

### 8.4 `spell` — Multi-Step Workflow

Like `kernel`, but semantically higher-level. Compiled identically to `kernel` (into the kernel table). The distinction is conceptual: kernels are low-level reasoning steps; spells are composed workflows.

```la
spell Summarise(text: str) -> str {
    observe(text);
    reason("Produce a concise summary");
    let result: str = infer(text);
    return result;
}
```

---

### 8.5 `memory` — Persistent Named Slot

Declares a named slot that persists across context resets and kernel invocations **within a single run**. Initialized once at program start.

```la
memory LastResult: str = "";
```

Memory slots are accessible as ordinary identifiers. Reads emit `LoadMemory(name)`; the initial value emits the expression followed by `AllocMemorySlot(name)`.

For **cross-run persistence**, use the `memory_load` / `memory_save` / `memory_delete` built-in primitives (see §4).

---

### 8.5.1 Cross-Run Persistent Memory

Three built-in functions provide access to a key-value store that survives program restarts when a persistent store is configured (e.g. via `lagent run --persist store.json`):

| Primitive | Description |
|---|---|
| `memory_load(key: str) -> str` | Return persisted value, or `""` if absent |
| `memory_save(key: str, value: str)` | Persist `value` under `key` |
| `memory_delete(key: str)` | Remove `key` from the store |

```la
fn main() {
    let count = memory_load("visits");
    memory_save("visits", "updated");
    println(count);
}
```

If no persistent store is attached to the VM, `memory_save` and `memory_delete` are silent no-ops and `memory_load` returns `""`.

---

### 8.6 `oracle` — External Lookup Stub

Declares an external knowledge source with a typed interface. The body is provided at runtime by the VM's backend. In Phase 4, the simulated backend returns `<oracle:Name>` as a placeholder.

```la
oracle FetchContext(url: str) -> str;

fn main() {
    let docs = FetchContext("https://example.com/data");
    println(docs);
}
```

Emits `CallOracle(name, arity)` at call sites. The backend receives the name and argument strings and returns a result string.

---

### 8.7 `constraint` — Named Guard Block

Declares a named verification rule. Use `apply ConstraintName;` to inline the constraint body at a call site. The `verify(...)` inside a constraint emits `ConstraintVerify`, which raises a non-retriable `ConstraintViolation` error — unlike kernel `verify(...)` which triggers the retry loop.

```la
constraint NonEmpty {
    verify(result != "");
}

skill Summarise(text: str) -> str {
    let result: str = infer(text);
    apply NonEmpty;
    return result;
}
```

At the bytecode level, `apply NonEmpty;` emits:
1. `BeginConstraint("NonEmpty")` — diagnostic marker.
2. The inlined constraint body (with `ConstraintVerify` instead of `VerifyStep`).
3. `EndConstraint` — diagnostic marker.

---

### 8.8 `lore` — Static Knowledge String

Declares a named static string injected into the VM's lore table at program start. Can be loaded onto the stack and appended to context.

```la
lore Background = "This agent analyses user-provided text for emotional tone.";
```

Emits `StoreLore(name, text)` during program initialization. Accessing `Background` as an identifier emits `LoadLore("Background")`.

---

## 9. Module System

### 9.1 Import

```la
use "utils/text.la";
```

The compiler's `resolver.rs` expands `use` declarations inline before semantic analysis. All items from the imported file are prepended to the importing file's item list. Imports are resolved recursively (transitive dependencies included).

Resolution rules:
- Paths are relative to the importing source file.
- Circular imports are not detected in Phase 4 (planned for Phase 5).
- `.lalb` (L-Agent Library Bundle) imports are planned for Phase 5.

### 9.2 Visibility

The `pub` modifier is supported on `fn`, `kernel`, `skill`, `spell`, `type`, `oracle`, `constraint`, `lore`. When a file is imported via `use`, only `pub` items are made visible in the importing scope. `SoulDef`, `MemoryDecl`, and `UseDecl` are never re-exported.

```la
pub skill AnalyseMood(text: str) -> Mood { ... }  // visible to importers
fn helper() { ... }                                // private — not exported
```

### 9.3 Library Declaration (`lagent.toml`)

```toml
[project]
name    = "my-agent"
version = "0.1.0"
entry   = "src/main.la"

[lib]
entry = "src/lib.la"
name  = "my-agent-lib"
```

`lagent build --lib` produces `my-agent-lib.lalb` — a precompiled bytecode bundle containing:
- The compiled kernel table and main instruction stream.
- An export table (`Vec<ExportEntry>`) listing only `pub` items.

Magic header: `b"LALB"` (distinct from executable `.lbc` files which use `b"LAGN"`).

---

## 10. Bytecode Instruction Set

See `lagent-compiler/src/codegen/opcodes.rs` for the full `OpCode` enum.

### Core
| Opcode | Description |
|--------|-------------|
| `Halt` | Stop execution |
| `Println` | Pop and print TOS |
| `Return` | Return from kernel frame |
| `PushStr(s)` | Push string literal |
| `PushInt(n)` | Push integer literal |
| `PushFloat(f)` | Push float literal |
| `StoreLocal(name)` | Pop TOS → local variable |
| `LoadLocal(name)` | Push local variable (falls back to memory slots) |

### Context
| Opcode | Description |
|--------|-------------|
| `CtxAlloc(n)` | Allocate context segment of `n` tokens |
| `CtxFreeStack` | Free context segment (handle on TOS) |
| `CtxAppendStack` | Append string to context segment (seg, str on stack) |
| `CtxCompress` | Summarize context segment via backend |
| `CtxShare` | Duplicate TOS context handle |
| `CtxAppendLiteral(s)` | Append literal string to in-scope ctx handle |

### Inference
| Opcode | Description |
|--------|-------------|
| `Observe` | Pop value and observe it (trace) |
| `Reason(hint)` | Emit reasoning hint (trace) |
| `Act` | Pop and execute action via backend |
| `VerifyStep` | Pop condition; retry on failure (up to MAX_KERNEL_RETRIES) |
| `InferClassify(labels)` | Pop prompt; classify among labels via backend |
| `BranchClassify { var, cases, default }` | Classify and jump to matching case body |

### Kernels & Calls
| Opcode | Description |
|--------|-------------|
| `CallKernel(idx)` | Call kernel/spell/skill by table index |
| `BeginInterruptible` | Save checkpoint |
| `EndInterruptible` | Discard checkpoint |

### Agent Vocabulary
| Opcode | Description |
|--------|-------------|
| `SetAgentMeta(s)` | Store soul identity string in VM |
| `RegisterSkill(name)` | Metadata marker (no runtime effect) |
| `AllocMemorySlot(name)` | Pop TOS → named in-run memory slot |
| `LoadMemory(name)` | Push in-run memory slot value |
| `StoreMemory(name)` | Pop TOS → in-run memory slot |
| `CallOracle(name, arity)` | Pop N args; call backend oracle; push result |
| `BeginConstraint(name)` | Mark constraint start (diagnostic) |
| `EndConstraint` | Mark constraint end (diagnostic) |
| `ConstraintVerify` | Pop TOS; non-retriable abort if falsy (`ConstraintViolation`) |
| `StoreLore(name, text)` | Store lore string in VM lore table |
| `LoadLore(name)` | Push lore string onto stack |

### Persistent Memory
| Opcode | Description |
|--------|-------------|
| `PersistLoad` | Pop key; push persisted value (or `""`) |
| `PersistSave` | Pop value then key; persist the pair |
| `PersistDelete` | Pop key; remove from persistent store |

### Arithmetic / Comparison
| Opcode | Description |
|--------|-------------|
| `CmpNotEq` | Pop two values; push bool (!=) |
| `CmpGt` | Pop two values; push bool (>) |
| `CmpLt` | Pop two values; push bool (<) |
