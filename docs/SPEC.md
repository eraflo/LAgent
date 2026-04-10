# L-Agent Language Specification

**Version 0.1 — April 2026**

## 1. Lexical Grammar

### 1.1 Keywords
```
fn kernel branch case default type let return
observe reason act verify infer
ctx_alloc ctx_free ctx_append ctx_resize
local_model_load local_model_infer local_model_unload local_model_list
println semantic intent
str bool u32 f32
```

### 1.2 Literals
- String literals: `"..."` with standard escape sequences
- Integer literals: `[0-9]+`
- Float literals: `[0-9]+\.[0-9]+`

### 1.3 Identifiers
`[a-zA-Z_][a-zA-Z0-9_]*`

### 1.4 Comments
Line comments: `// ...`

## 2. Grammar (EBNF)

```ebnf
program     = item* ;
item        = fn_def | kernel_def | type_alias ;

fn_def      = "fn" IDENT "(" params ")" ("->" type_expr)? block ;
kernel_def  = "kernel" IDENT "(" params ")" "->" type_expr block ;
type_alias  = "type" IDENT "=" type_expr ";" ;

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
            | expr_stmt ;

let_stmt    = "let" IDENT (":" type_expr)? "=" expr ";" ;
return_stmt = "return" expr ";" ;
branch_stmt = "branch" IDENT "{" branch_case* ("default" "=>" block)? "}" ;
branch_case = "case" STRING "(" "confidence" ">" FLOAT ")" "=>" block ;
expr_stmt   = expr ";" ;

expr        = call_expr | IDENT | STRING | INT | FLOAT | bin_expr ;
call_expr   = IDENT "(" (expr ("," expr)*)? ")" ;
bin_expr    = expr bin_op expr ;
bin_op      = "!=" | ">" | "<" ;
```

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

A semantic type defines a named set of concepts. At runtime, assignment to a semantic type variable is validated by measuring cosine distance in the embedding space of the active model. A value is accepted if its distance to at least one concept is below the configured threshold.

### 3.3 Context Segments
`CtxSegment` is a first-class resource type returned by `ctx_alloc`. It represents a named window into the LLM's context. Must be explicitly freed with `ctx_free`.

## 4. Context Primitives

| Primitive                               | Description                                  |
|-----------------------------------------|----------------------------------------------|
| `ctx_alloc(tokens: u32) -> CtxSegment`  | Allocate a context segment                   |
| `ctx_free(seg: CtxSegment)`             | Free a context segment                       |
| `ctx_append(seg: CtxSegment, s: str)`   | Append text to a segment                     |
| `ctx_resize(seg: CtxSegment, n: u32)`   | Resize a segment                             |
| `ctx_compress(seg: CtxSegment)`         | Summarize segment content to reclaim tokens  |

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

## 7. Bytecode Instruction Set

See `lagent-compiler/src/codegen/opcodes.rs` for the full `OpCode` enum.

| Opcode                            | Description                            |
|-----------------------------------|----------------------------------------|
| `CtxAlloc(n)`                     | Allocate context segment               |
| `CtxFree(reg)`                    | Free context segment                   |
| `CtxAppend(seg_reg, str_reg)`     | Append string to segment               |
| `PushStr(s)`, `PushInt(n)`, ...   | Push literals                          |
| `Call(name)`                      | Call named function                    |
| `CallKernel(idx)`                 | Call kernel by index                   |
| `Branch { cases, default }`       | Probabilistic branch                   |
| `LocalInfer(dst, model, prompt)`  | Run local inference                    |
| `Return`                          | Return from function                   |
| `Println`                         | Print top of stack                     |
| `Halt`                            | Stop execution                         |
