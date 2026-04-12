# L-Agent Language Specification

**Version 0.6 — April 2026**

This document defines the syntax and semantics of L-Agent (`.la` files). It covers the current implementation (Phases 1–6) and marks planned future features with their target phase.

---

## 1. Lexical Structure

### 1.1 Keywords

```
fn          kernel      branch      case        default
type        let         return      pub         use
interruptible apply     observe     reason      act
verify      infer       soul        skill       spell
instruction memory      persistent  oracle      constraint
lore

ctx_alloc   ctx_free    ctx_append  ctx_resize  ctx_compress
ctx_share

println     semantic    str         bool        u32         f32
```

### 1.2 Literals

| Kind | Syntax | Example |
|------|--------|---------|
| String | `"..."` with `\n`, `\t`, `\\`, `\"` escapes | `"hello\nworld"` |
| Integer | `[0-9]+` | `42`, `0` |
| Float | `[0-9]+\.[0-9]+` | `3.14`, `0.5` |

### 1.3 Identifiers

```
[a-zA-Z_][a-zA-Z0-9_]*
```

Identifiers are case-sensitive. `myVar`, `my_var`, and `MyVar` are distinct.

### 1.4 Comments

```la
// Single-line comment — everything from // to end of line is ignored
```

Block comments (`/* ... */`) are not currently supported.

### 1.5 Operators

| Operator | Meaning |
|----------|---------|
| `!=` | Not equal |
| `>` | Greater than |
| `<` | Less than |

> **Planned (Phase 7):** `==`, `>=`, `<=`, `&&`, `||`, `!`, `+`, `-`, `*`, `/`, `%`

---

## 2. Type System

### 2.1 Primitive Types

| Type | Description |
|------|-------------|
| `str` | UTF-8 string |
| `bool` | Boolean (`true` / `false` — currently represented as string literals) |
| `u32` | 32-bit unsigned integer |
| `f32` | 32-bit floating-point number |

### 2.2 Semantic Types

```la
type Mood = semantic("happy", "sad", "neutral");
```

A semantic type defines a **named, closed set of labels**. At runtime, `infer` with a semantic return type performs **constrained classification** among the declared labels using the backend's `classify` method.

**Rules:**
- Semantic types are declared with `type Name = semantic("label1", "label2", ...);`
- Labels are string literals
- The set is closed — no other values are valid
- At runtime, the backend returns one of the declared labels

**Example:**
```la
type Priority = semantic("low", "medium", "high");

fn get_label() -> Priority {
    let p: Priority = infer("What is the priority?");
    return p;  // guaranteed to be "low", "medium", or "high"
}
```

### 2.3 Context Segments

`CtxSegment` is a first-class resource type returned by `ctx_alloc`. It represents a handle into the Token Heap (the LLM context window manager). Must be explicitly freed with `ctx_free`.

See [`TOKEN_HEAP.md`](TOKEN_HEAP.md) for the complete design.

### 2.4 Future Types (Planned)

| Feature | Phase | Syntax |
|---------|-------|--------|
| Tuples | 7 | `(str, u32)`, `tuple.0` indexing |
| Structs | 7 | `struct User { name: str, age: u32 }` |
| Enums | 7 | `enum Status { Ok, Err(str) }` |
| Collections | 7 | `Vec<T>`, `Map<K, V>`, `Set<T>` |
| Generics | 10 | `fn map<T, U>(...)` |

---

## 3. Declarations

### 3.1 `fn` — Pure Function

```la
fn name(params) -> ReturnType? { body }
```

A classic programming function with **no access to LLM or context primitives**.

**Compile-time restrictions:**
- ❌ Cannot use: `ctx_*`, `infer`, `branch`, `observe`, `reason`, `act`, `verify`
- ❌ Cannot call: `kernel`, `spell`
- ✅ Can call: other `fn`, `skill`

**Runtime behavior:** Standard function call. No LLM interaction, no context manipulation.

**Example:**
```la
fn add_tax(price: f32, rate: f32) -> f32 {
    return price * (1.0 + rate);
}
```

> **Planned (Phase 7):** `if/else`, `loop`, `while`, `for` inside function bodies.

---

### 3.2 `kernel` — Transactional Reasoning Unit

```la
kernel Name(params) -> ReturnType { body }
```

A unit of reasoning with a **structured lifecycle**: `observe` → `reason` → `act` → `verify`.

**Compile-time rules:**
- Return type is **mandatory**
- Body should contain `observe`, `reason`, `act`, `verify` in that order (currently enforced by convention, strictly enforced in Phase 5)

**Runtime behavior:**
1. A new **kernel frame** is pushed with bound parameters
2. Each step is traced in the execution log
3. If `verify` fails, the kernel **retries** from the start (up to `MAX_KERNEL_RETRIES`, default: 3)
4. If all retries are exhausted, a `KernelVerifyError` is raised
5. On success, the frame is popped and the return value is pushed

**Example:**
```la
kernel AnalyseSentiment(text: str) -> Sentiment {
    observe(text);
    reason("Classify the sentiment of the input text");
    let result: Sentiment = infer(text);
    verify(result != "");
    return result;
}
```

> **Planned (Phase 5):** Strict section ordering enforced at compile-time. Private `CtxSegment` allocated per kernel invocation for isolation. Rollback on verify failure.

---

### 3.3 `skill` — Agent Capability

```la
[pub] skill Name(params) -> ReturnType? { body }
```

A callable agent capability. Compiled into the kernel table (same as `kernel` and `spell`).

**Current behavior:** Same as `kernel` — can use all kernel primitives (`observe`, `reason`, `infer`, `verify`, etc.).

**Return type:** Optional (may be omitted).

**Example:**
```la
pub skill AnalyseMood(text: str) -> Mood {
    observe(text);
    reason("Classify the mood of the text");
    let result: Mood = infer(text);
    verify(result != "");
    return result;
}
```

> **Planned (Phase 5):** Compile-time restrictions — `skill` cannot use `ctx_*`, `infer`, `branch`, `observe`, `reason`, `act`, `verify`. Can only call other `skill` or `fn`. The `#[tool]` attribute will mark skills as safe for LLM function calling.

---

### 3.4 `spell` — Multi-Step Workflow

```la
spell Name(params) -> ReturnType { body }
```

A higher-level workflow. Compiled identically to `kernel` (into the kernel table).

**Conceptual distinction:** Kernels are low-level reasoning steps; spells are composed workflows.

**Return type:** Mandatory.

**Example:**
```la
spell Summarise(text: str) -> str {
    observe(text);
    reason("Produce a concise summary");
    let result: str = infer(text);
    return result;
}
```

---

### 3.5 `type` — Type Alias

```la
type Name = TypeExpr;
```

Creates a named alias for a type expression. Currently used exclusively for semantic types.

**Example:**
```la
type Emotion = semantic("joie", "colère", "tristesse", "neutre");
```

---

### 3.6 `memory` — Intra-Run Persistent Slot

```la
memory Name: Type = initial_value;
```

Declares a named slot that persists across context resets and kernel invocations **within a single run**. Initialized once at program start.

**Scope:** Global within the program execution.
**Lifetime:** Lost on program exit.

**Example:**
```la
memory LastResult: str = "";
```

Access reads/writes through `LoadMemory` / `StoreMemory` opcodes.

---

### 3.7 `persistent memory` — Inter-Run Persistent Slot *(Planned: Phase 7)*

```la
persistent memory Name: Type = default_value;
```

Like `memory`, but **survives program restarts** when a persistent store is attached (`lagent run --persist store.json`).

**Behavior:**
- On first run: initialized with `default_value`
- On subsequent runs: loaded from the persistent store
- The compiler generates automatic serialization/deserialization code

> **Current (Phase 6):** Cross-run persistence uses `memory_load` / `memory_save` / `memory_delete` built-in primitives. These will be deprecated in favor of `persistent memory` syntax.

---

### 3.8 `oracle` — External Lookup Stub

```la
oracle Name(params) -> ReturnType;
```

Declares an external knowledge source with a typed interface. The implementation is provided at runtime by the VM's backend.

**Semantics:**
- No body in source code — resolved dynamically
- Calls emit `CallOracle(name, arity)` opcode
- The backend receives the name and argument strings, returns a result string

**Example:**
```la
oracle FetchContext(url: str) -> str;

fn main() {
    let docs = FetchContext("https://example.com/data");
    println(docs);
}
```

> **Planned (Phase 11):** Distinguish `oracle` (backend-mediated, non-deterministic external calls) from `extern` (direct FFI to native Rust/C code, deterministic).

---

### 3.9 `constraint` — Named Guard Block

```la
constraint Name { verify(expr); }
```

Declares a reusable verification rule. Applied via `apply Name;`.

**Semantics:**
- Body is limited to `verify(expr)` and `apply` of other constraints
- No side effects allowed
- At `apply` site, the body is inlined
- `verify` inside a constraint emits `ConstraintVerify` — **non-retriable** (unlike kernel `verify` which retries)
- Failure raises `ConstraintViolation` — fatal error

**Example:**
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

**Bytecode emitted at `apply` site:**
1. `BeginConstraint("NonEmpty")` — diagnostic marker
2. Inlined body with `ConstraintVerify`
3. `EndConstraint` — diagnostic marker

---

### 3.10 `lore` — Static Knowledge String

```la
lore Name = "text content";
```

Declares a named string **injected into the VM at startup**. Unlike `const`, `lore` is **visible to the LLM** — it is loaded into the system context during program initialization.

**Semantics:**
- Emits `StoreLore(name, text)` during initialization
- Accessing the identifier emits `LoadLore(name)` — pushes the string onto the stack
- Read-only at runtime

**Example:**
```la
lore COMPANY_POLICY = "Returns are accepted within 30 days.";
```

> **Distinction from `const` (Phase 7):** `const` is a compile-time value used in expressions, **never visible to the LLM**. `lore` is runtime knowledge injected into the agent's context, **always visible to the LLM**.

---

### 3.11 `const` — Compile-Time Constant *(Planned: Phase 7)*

```la
const MAX_TOKENS: u32 = 4096;
```

A value **evaluated at compile time** and inlined at use sites. Never appears in the bytecode runtime.

**Rules:**
- Only primitive types (`u32`, `f32`, `str`, `bool`)
- Evaluated once by the compiler
- Not visible to the LLM
- Usable in expressions, array sizes, conditionals

---

### 3.12 `use` — Module Import

```la
use "path.la";
```

Imports another `.la` file. The compiler's resolver expands imports **inline before semantic analysis**.

**Resolution rules:**
- Paths are relative to the importing source file
- Imports are resolved recursively (transitive dependencies)
- Only `pub` items from imported files are visible in the importing scope
- `SoulDef`, `MemoryDecl`, and `UseDecl` are never re-exported

**Example:**
```la
// utils.la
pub skill Helper(text: str) -> str { ... }
fn private_helper() { ... }

// main.la
use "utils.la";
// Helper is visible, private_helper is not
```

---

### 3.13 `pub` — Visibility Modifier

```la
pub fn name(...) { ... }
pub skill name(...) { ... }
pub type Name = ...;
```

Makes an item exportable across module boundaries. When a file is imported via `use`, only `pub` items are visible.

**Applicable to:** `fn`, `kernel`, `skill`, `spell`, `type`, `oracle`, `constraint`, `lore`.

> **Planned (Phase 11):** `pub(crate)`, `pub(super)`, `pub(self)`, `pub(in path)` for scoped visibility.

---

## 4. Statements

### 4.1 `let` — Local Binding

```la
let name: Type? = expr;
```

Declares a local variable in the current frame. Type annotation is optional (inferred from the expression).

**Example:**
```la
let result: Mood = infer(text);
let ctx = ctx_alloc(1024);
```

> **Planned (Phase 7):** `let mut name = expr;` for explicit mutable bindings. Default `let` is immutable.

---

### 4.2 `return` — Function Return

```la
return expr;
```

Returns a value from the current frame. In kernels, `return` is only valid inside the `act` section.

---

### 4.3 `branch` — Probabilistic Branching

```la
branch variable {
    case "label" (confidence > threshold) => { body }
    default => { body }
}
```

A control-flow construct driven by LLM classification.

**Semantics:**
1. The runtime classifies the scrutinee variable among case labels using constrained decoding
2. Cases are evaluated **in order**
3. The first case whose confidence exceeds the threshold is executed
4. If no case matches, the `default` block runs
5. If `default` is absent and no case matches, execution continues silently

**Example:**
```la
branch intent {
    case "angry" (confidence > 0.7) => {
        println("Gestion de crise activée");
    }
    case "help" (confidence > 0.4) => {
        println("Support standard");
    }
    default => {
        println("Redirection vers un opérateur humain");
    }
}
```

---

### 4.4 `interruptible` — Interruptible Block

```la
interruptible { body }
```

A safe interaction point. On entry, a **checkpoint** of the current frame is saved.

**Semantics:**
- If an error occurs inside the block, the frame is restored to the checkpoint
- Execution resumes after the block
- Used for long-running operations that may be interrupted

**Example:**
```la
interruptible {
    println(sentiment);
    let response = GenerateResponse(ctx);
}
```

---

### 4.5 `instruction` — System Directive

```la
instruction "text";
```

Appends a literal string to the active context handle.

**Current behavior:** Valid inside `soul` and `skill` bodies.

> **Planned (Phase 7):** Restricted to `soul` blocks only. Using `instruction` outside `soul` will be a compile error.

---

### 4.6 `apply` — Apply Constraint

```la
apply ConstraintName;
```

Inlines the body of a named constraint at the current location.

**Rules:**
- The constraint must be defined and visible in scope
- Emits `BeginConstraint`, inlined body with `ConstraintVerify`, `EndConstraint`
- Non-retriable — failure is fatal

---

### 4.7 Expression Statement

```la
expr;
```

Any expression followed by a semicolon. The result is discarded.

**Example:**
```la
ctx_append(ctx, "additional context");
println(result);
```

---

## 5. Expressions

### 5.1 Literals

```la
"hello world"    // str
42               // u32
3.14             // f32
```

### 5.2 Function Call

```la
name(arg1, arg2, ...)
```

Calls a function, skill, spell, or kernel. Arguments are evaluated left-to-right.

### 5.3 Binary Operations

```la
expr != expr     // not equal
expr > expr      // greater than
expr < expr      // less than
```

> **Planned (Phase 7):** `==`, `>=`, `<=`, `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `!`

### 5.4 Identifier

```la
variable_name
```

Resolves to a local variable, memory slot, lore entry, or oracle.

---

## 6. Agent Vocabulary

These keywords form L-Agent's **declarative agent identity system**:

| Keyword | Role | Compiled Into |
|---------|------|---------------|
| `soul` | Agent identity and system prompt | Soul preamble (injected before `fn main`) |
| `skill` | Callable capability | Kernel table (`CallKernel`) |
| `spell` | Multi-step workflow | Kernel table (`CallKernel`) |
| `kernel` | Transactional reasoning unit | Kernel table (`CallKernel`) |
| `memory` | Intra-run persistent slot | Memory slots (`LoadMemory`/`StoreMemory`) |
| `oracle` | External capability stub | Backend oracle (`CallOracle`) |
| `constraint` | Named verification rule | Inlined at `apply` site |
| `lore` | Static knowledge string | Lore table (`StoreLore`/`LoadLore`) |

### 6.1 `soul` — Agent Identity

```la
soul {
    instruction "You are a helpful assistant.";
    instruction "Always respond concisely.";
}
```

The `soul` block defines the agent's identity. Each `instruction` statement appends to the system context before `fn main` executes.

**Bytecode:** `SetAgentMeta("soul")` → `CtxAppendLiteral` for each instruction.

**Rules:**
- Only one `soul` block per program
- Contains only `instruction` statements

> **Planned (Phase 7):** `instruction` restricted to `soul` blocks only. Using it elsewhere is a compile error.

---

## 7. Context Management

Context primitives provide explicit control over the LLM's context window (the Token Heap).

| Primitive | Signature | Description |
|-----------|-----------|-------------|
| `ctx_alloc` | `(tokens: u32) -> CtxSegment` | Allocate a context segment |
| `ctx_free` | `(seg: CtxSegment)` | Free a context segment |
| `ctx_append` | `(seg: CtxSegment, text: str)` | Append text to a segment |
| `ctx_resize` | `(seg: CtxSegment, tokens: u32)` | Resize a segment's capacity |
| `ctx_compress` | `(seg: CtxSegment)` | Summarize content to reclaim tokens |
| `ctx_share` | `(seg: CtxSegment) -> CtxSegment` | Duplicate a segment handle |

**Lifecycle:**
```la
let ctx = ctx_alloc(4096);    // allocate
ctx_append(ctx, "Hello");     // use
ctx_free(ctx);                 // free — must be called exactly once
```

See [`TOKEN_HEAP.md`](TOKEN_HEAP.md) for the complete design, planned extensions, and best practices.

---

## 8. Module System

### 8.1 Import

```la
use "path.la";
```

Expands the contents of `path.la` inline. Paths are relative to the importing file. Resolution is recursive and transitive.

### 8.2 Visibility

Items are private by default. The `pub` modifier makes them visible to importing modules.

### 8.3 Library Bundles (`.lalb`)

```bash
lagent build --lib src/lib.la    # → my-agent-lib.lalb
```

A precompiled bytecode bundle containing:
- The compiled kernel table and instruction stream
- An export table listing `pub` items

Magic header: `b"LALB"` (distinct from `.lbc` which uses `b"LAGN"`).

---

## 9. Error Model

### 9.1 Compile-Time Errors

| Error | Cause | Example |
|-------|-------|---------|
| Lexical error | Invalid character, unterminated string | `"unclosed` |
| Parse error | Syntax violation | `fn () { }` (no name) |
| Name resolution | Undefined identifier | `println(undefined_var)` |
| Type error | Type mismatch | `let x: Mood = 42;` |
| Duplicate name | Same identifier declared twice | `fn foo() {}` then `fn foo() {}` |
| Invalid apply | Unknown constraint name | `apply UnknownConstraint;` |

### 9.2 Runtime Errors

| Error | Cause | Recovery |
|-------|-------|----------|
| `HeapError::Overflow` | Context budget exceeded | Fatal |
| `HeapError::InvalidHandle` | Use-after-free or invalid segment ID | Fatal |
| `KernelVerifyError` | Kernel verify failed after all retries | Fatal |
| `ConstraintViolation` | Constraint check failed | Fatal (non-retriable) |
| Backend error | Network failure, API error | Depends on backend implementation |

---

## Appendix A: Complete EBNF Grammar

```ebnf
program     = item* ;

item        = fn_def
            | kernel_def
            | skill_def
            | spell_def
            | type_alias
            | soul_def
            | memory_decl
            | oracle_decl
            | constraint_def
            | lore_decl
            | use_decl ;

fn_def          = "fn" IDENT "(" params ")" ("->" type_expr)? block ;
kernel_def      = "kernel" IDENT "(" params ")" "->" type_expr block ;
skill_def       = "pub"? "skill" IDENT "(" params ")" ("->" type_expr)? block ;
spell_def       = "spell" IDENT "(" params ")" "->" type_expr block ;
type_alias      = "type" IDENT "=" type_expr ";" ;
soul_def        = "soul" block ;
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

## Appendix B: Bytecode Instruction Set

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
| `RegisterSkill(name)` | Metadata marker (no runtime effect — planned for removal in Phase 7) |
| `AllocMemorySlot(name)` | Pop TOS → named in-run memory slot |
| `LoadMemory(name)` | Push in-run memory slot value |
| `StoreMemory(name)` | Pop TOS → in-run memory slot |
| `CallOracle(name, arity)` | Pop N args; call backend oracle; push result |
| `BeginConstraint(name)` | Mark constraint start (diagnostic) |
| `EndConstraint` | Mark constraint end (diagnostic) |
| `ConstraintVerify` | Pop TOS; non-retriable abort if falsy (`ConstraintViolation`) |
| `StoreLore(name, text)` | Store lore string in VM lore table |
| `LoadLore(name)` | Push lore string onto stack |

### Persistent Memory (Cross-Run)

| Opcode | Description |
|--------|-------------|
| `PersistLoad` | Pop key; push persisted value (or `""`) — planned for removal in Phase 7 |
| `PersistSave` | Pop value then key; persist the pair — planned for removal in Phase 7 |
| `PersistDelete` | Pop key; remove from persistent store — planned for removal in Phase 7 |

### Arithmetic / Comparison

| Opcode | Description |
|--------|-------------|
| `CmpNotEq` | Pop two values; push bool (!=) |
| `CmpGt` | Pop two values; push bool (>) |
| `CmpLt` | Pop two values; push bool (<) |
