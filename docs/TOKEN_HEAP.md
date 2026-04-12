# The Token Heap

> L-Agent's context window memory system.

## Overview

In C, you manage memory with `malloc` and `free`. In L-Agent, you manage **LLM context tokens** with `ctx_alloc` and `ctx_free`. The **Token Heap** is the runtime subsystem that makes this possible.

```
┌─────────────────────────────────────────────────────┐
│                    Token Heap                        │
│                                                      │
│  total_capacity: 8192 tokens                         │
│  used:           3072 tokens                         │
│  available:      5120 tokens                         │
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ Segment #0   │  │ Segment #1   │  │ Segment #2 │ │
│  │ capacity:2048│  │ capacity:1024│  │cap:  4096  │ │
│  │ content:"..."│  │ content:"..."│  │ content:"" │ │
│  └──────────────┘  └──────────────┘  └────────────┘ │
│                                                      │
└─────────────────────────────────────────────────────┘
```

The Token Heap is a **slab allocator** for context segments. Each segment has a unique handle (`u32`), a capacity in tokens, and a text content buffer.

---

## Analogy with C Memory Management

| C Concept | Token Heap Equivalent | Notes |
|-----------|----------------------|-------|
| `malloc(size)` | `ctx_alloc(tokens) → id` | Returns a segment handle |
| `free(ptr)` | `ctx_free(id)` | Frees by handle, not pointer |
| `realloc(ptr, new_size)` | `ctx_resize(id, new_tokens)` | *(planned)* |
| Memory leak | Context token exhaustion | `HeapError::Overflow` |
| Use-after-free | `HeapError::InvalidHandle` | Detected at runtime |
| Double-free | `HeapError::InvalidHandle` | Detected at runtime |
| `heap_usage()` | `heap.used()` | Returns currently used tokens |

---

## Current Primitives

### `ctx_alloc(tokens: u32) -> CtxSegment`

Allocates a new context segment with the given token capacity.

**Compile-time rules:**
- Argument must be a `u32` expression
- Returns a `CtxSegment` handle

**Runtime behavior:**
1. Checks if `tokens <= (total_capacity - used)`
2. If insufficient: returns `HeapError::Overflow` → fatal VM error
3. Otherwise: creates a new `CtxSegment` with empty content, increments `used`
4. Returns the segment's unique `id`

**Example:**
```la
let ctx = ctx_alloc(4096);  // allocate 4096-token segment
```

**Implementation:** O(n) — scans the segment vector for insertion point. `used` is incremented by the requested capacity.

---

### `ctx_free(seg: CtxSegment)`

Frees a previously allocated context segment.

**Compile-time rules:**
- Argument must be a `CtxSegment` expression

**Runtime behavior:**
1. Finds the segment by `id` in the segment vector
2. If not found: returns `HeapError::InvalidHandle` → fatal VM error
3. Removes the segment, decrements `used` by the segment's capacity
4. The segment's content is discarded

**Example:**
```la
ctx_free(ctx);  // must be called exactly once per alloc
```

**Safety:** The handle becomes invalid immediately after `ctx_free`. Any subsequent use of that handle will raise `InvalidHandle`.

---

### `ctx_append(seg: CtxSegment, text: str)`

Appends text to a context segment's content buffer.

**Runtime behavior:**
1. Validates the segment handle
2. Appends text to the segment's `content` string
3. No token counting is performed at append time (capacity is advisory)

**Example:**
```la
ctx_append(ctx, "You are a helpful assistant.");
ctx_append(ctx, "\nUser: Hello!");
```

---

### `ctx_resize(seg: CtxSegment, tokens: u32)`

Changes a segment's capacity.

**Runtime behavior:**
- If new capacity > old capacity: checks available space in the heap
- If new capacity < old capacity: content is not truncated (capacity is advisory)
- Adjusts `used` accordingly

---

### `ctx_compress(seg: CtxSegment)`

Summarizes a segment's content to reclaim tokens.

**Runtime behavior:**
1. Sends the current content to the backend's `compress` method
2. Replaces content with the summarized version
3. Capacity remains unchanged; the smaller content implicitly uses fewer effective tokens

**Example:**
```la
ctx_compress(ctx);  // summarize long content
```

---

### `ctx_share(seg: CtxSegment) -> CtxSegment`

Duplicates a context segment handle reference.

**Runtime behavior:**
- Returns a new handle pointing to the same underlying segment
- Both handles share the same content

> **Planned (Phase 11):** Copy-on-Write semantics — writes to a shared segment trigger lazy duplication.

**Example:**
```la
let shared = ctx_share(ctx);
ctx_append(ctx, "added to shared segment");
// shared sees the same content
```

---

## Lifecycle Diagram

```
                    ┌─────────────┐
                    │   Program   │
                    │    Start    │
                    └──────┬──────┘
                           │
                    ctx_alloc(N)
                           │
                           ▼
                    ┌─────────────┐
                    │  ALLOCATED  │ ◄──┐
                    │  (valid)    │    │
                    └──────┬──────┘    │
                           │           │
              ┌────────────┼───────────┤
              │            │           │
      ctx_append    ctx_compress   ctx_share
      (add text)   (summarize)    (duplicate)
              │            │           │
              └────────────┼───────────┘
                           │
                      ctx_free()
                           │
                           ▼
                    ┌─────────────┐
                    │    FREED    │
                    │ (invalid)   │
                    └─────────────┘
                           │
              Any use → InvalidHandle error (fatal)
```

---

## Memory Safety

### Double-Free Detection

```la
let ctx = ctx_alloc(1024);
ctx_free(ctx);
ctx_free(ctx);  // ERROR: HeapError::InvalidHandle — segment already freed
```

The VM searches the segment vector by ID. If not found, it returns `InvalidHandle`.

### Use-After-Free Detection

```la
let ctx = ctx_alloc(1024);
ctx_free(ctx);
ctx_append(ctx, "hello");  // ERROR: HeapError::InvalidHandle — segment is freed
```

Same mechanism — the handle no longer maps to a live segment.

### Leak Detection

```la
fn main() {
    let ctx = ctx_alloc(4096);
    // forgot ctx_free(ctx)
}
```

Currently **not detected** at runtime. The VM exits with remaining `used` tokens unreported.

> **Planned (Phase 7):** Warning at VM exit if `used > 0` — potential context leak detected.

### Overflow Protection

```la
let ctx = ctx_alloc(8193);  // ERROR: HeapError::Overflow — requested > total_capacity
```

The heap tracks `total_capacity` vs `used`. Allocation is rejected if insufficient.

---

## Token Budgeting: Why Explicit Management Matters

LLMs have a **fixed context window** (e.g., 8K, 32K, 128K tokens). Every token costs money (API) or compute (local). The Token Heap gives you:

| Benefit | Without Token Heap | With Token Heap |
|---------|-------------------|-----------------|
| Budget awareness | Implicit, until it overflows | Explicit — you choose the size |
| Cost control | Unbounded until error | Bounded by `total_capacity` |
| Multi-agent isolation | Shared, risk of interference | Separate segments per agent |
| Compression | Manual string manipulation | `ctx_compress` via backend |
| Sharing | Copy-paste content | `ctx_share` with CoW (planned) |

**Example — multi-agent isolation:**
```la
// Two independent agents working in parallel
let agent_a_ctx = ctx_alloc(2048);
let agent_b_ctx = ctx_alloc(2048);

ctx_append(agent_a_ctx, "Agent A's context");
ctx_append(agent_b_ctx, "Agent B's context");

// Neither can see the other's content
```

---

## Implementation Details

### Data Structures

```rust
pub struct CtxSegment {
    pub id: u32,          // unique handle
    pub capacity: usize,  // token budget for this segment
    pub content: String,  // actual text content
}

pub struct TokenHeap {
    segments: Vec<CtxSegment>,  // live segments
    next_id: u32,               // monotonic ID counter
    total_capacity: usize,      // global token budget
    used: usize,                // currently allocated tokens
}
```

### Allocation Strategy

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `alloc` | O(n) | Appends to vector, checks capacity |
| `free` | O(n) | Linear search by ID, then `Vec::remove` |
| `append` | O(1) amortized | String push |
| `get` | O(n) | Linear search by ID |

> **Planned (Phase 11):** Hash map index for O(1) lookups by ID.

### Capacity Tracking

The Token Heap uses **advisory capacity**:
- `alloc(N)` reserves N tokens of the global budget
- `append` does not count tokens — the capacity is a budget, not a hard limit
- `compress` replaces content but keeps the same capacity

This means: the heap prevents you from **allocating too many segments**, but doesn't prevent you from **filling a segment with too much text**. Token counting is the programmer's responsibility.

---

## Bytecode Mapping

| Language Primitive | Opcode | Heap Method |
|-------------------|--------|-------------|
| `ctx_alloc(n)` | `CtxAlloc(n)` | `heap.alloc(n)` |
| `ctx_free(seg)` | `CtxFreeStack` | `heap.free(id_from_stack)` |
| `ctx_append(seg, text)` | `CtxAppendStack` | `heap.append(id, text)` |
| `ctx_compress(seg)` | `CtxCompress` | `backend.compress(heap.get_content(id))` |
| `ctx_share(seg)` | `CtxShare` | Clone handle reference |

---

## Best Practices

### 1. Always Pair `ctx_alloc` with `ctx_free`

```la
// ✅ Good — explicit lifecycle
let ctx = ctx_alloc(4096);
ctx_append(ctx, "Hello");
let result = AnalyseMood(ctx);
ctx_free(ctx);

// ❌ Bad — leak
let ctx = ctx_alloc(4096);
ctx_append(ctx, "Hello");
// forgot ctx_free — tokens lost until program exit
```

### 2. Use Minimal Necessary Capacity

```la
// ✅ Good — sized for the task
let ctx = ctx_alloc(1024);    // short classification
let big_ctx = ctx_alloc(8192);  // long generation

// ❌ Bad — wasteful
let ctx = ctx_alloc(32768);   // more than needed
```

### 3. Compress Before Growing

```la
// ✅ Good — compress before allocating more
ctx_compress(ctx);
let new_ctx = ctx_alloc(2048);  // now has room

// ❌ Bad — will overflow
let ctx = ctx_alloc(4096);
// fill it up...
let new_ctx = ctx_alloc(4096);  // Overflow if total < 8192
```

### 4. Share Context When Agents Need Shared State

```la
// ✅ Good — shared history
let base = ctx_alloc(4096);
ctx_append(base, "System: You are part of a team.");
let agent_a = ctx_share(base);
let agent_b = ctx_share(base);
// Both agents see the system message
```

### 5. Use `ctx_append` Incrementally, Not in One Giant String

```la
// ✅ Good — composable
ctx_append(ctx, "System prompt");
ctx_append(ctx, "\nUser: ");
ctx_append(ctx, user_input);

// ❌ Bad — harder to reason about
ctx_append(ctx, "System prompt\nUser: hello world");
```

---

## Planned Extensions

### Phase 10–12 Roadmap

| Feature | Syntax | Description |
|---------|--------|-------------|
| **Context Views** | `ctx_view(seg)` / `&CtxView` | Immutable reference to segment; compile-time non-mutation guarantees |
| **View Slicing** | `ctx_view_slice(view, start, len)` | Create sub-view without copying tokens |
| **View Inspection** | `inspect(view) -> str` | Returns exact tokenized representation for debugging |
| **Context Guard** | `CtxGuard::new(ctx)` | RAII pattern for automatic context rollback on scope exit |
| **Copy-on-Write** | *(automatic on `ctx_share`)* | Writes trigger lazy duplication, not shared mutation |
| **Memory-Mapped Context** | *(automatic for large segments)* | Page tokens from disk on demand for million-token contexts |
| **Context Swapping** | `ctx_swap_out(seg) → SwapId` / `ctx_swap_in(SwapId)` | Offload segments to disk, reload later |
| **Context Versioning** | `ctx_revert(version)` | Each write creates a version; rollback supported |
| **Semantic GC** | `ctx_alloc_managed()` | Reference-counted segments, auto-free when unreferenced |
| **Context Diff** | `ctx.diff(v1, v2) → Diff` | Semantic diff between two versions |
| **Pagination** | `ctx.next_page()` / `ctx.prev_page()` | Iterate over massive contexts without full load |
| **Merge** | `ctx_merge(seg1, seg2) → CtxSegment` | Combine two segments into one |
| **Clear** | `ctx_clear(seg)` | Empty segment without deallocating (prevents fragmentation) |
| **Inspection** | `ctx_len(seg) → u32` / `ctx_capacity(seg) → u32` | Query token usage and capacity |

### Design Vision: Context Views — Immutable by Default

```la
let big = ctx_alloc(8192);
ctx_append(big, "A very long document...");

// Create an immutable view — no copying, compile-time read-only
let intro = ctx_view(big);
let first_part = ctx_view_slice(intro, 0, 256);

// Function receiving &CtxView CANNOT mutate the segment
fn AnalyseMood(view: &CtxView) {
    let mood = infer(first_part, "What is the mood?");
    // ctx_append(view, "test")  // ❌ Compile error: view is immutable
    inspect(view)  // ✅ Debug: see exact tokenized content
}

AnalyseMood(first_part);
// big is unchanged and unmodified
```

#### Compiler Guarantees
- Functions declaring `view: &CtxView` promise non-mutation (like `const` in C)
- Passing `&CtxView` to a function expecting `CtxSegment` (mutable) → **type error**
- `infer`, `branch`, `observe` accept `&CtxView` natively
- No implicit coercion: `CtxView` ≠ `CtxSegment`

#### VM Opcodes
- `CtxViewCreate` — create immutable view from segment
- `CtxViewSlice` — create sub-view
- `CtxViewInspect` — push tokenized string of view onto stack (powers `inspect()`)
- `CtxViewFree` — release view handle

### Design Vision: Context Guard (CtxGuard) — RAII Pattern

#### The Problem
In LLM agent programming, context (the prompt) is cumulative: each `ctx.append()` adds text visible to all subsequent inferences. To avoid polluting the conversation with temporary data (a document to analyze, a hypothesis to test), developers must currently manage manual save/restore, which is error-prone and verbose.

#### The Solution
`CtxGuard` is a **type** (not a keyword) that applies the **RAII** pattern (Resource Acquisition Is Initialization) to the Token Heap.

- **Constructor**: `CtxGuard::new(ctx)` takes a context segment and saves its current state (write position).
- **Destructor**: When the `CtxGuard` object goes out of scope (end of `{ }` block or function), its destructor is automatically called. It restores the context to the saved state, erasing all modifications made during its lifetime.

```la
fn analyser_document(texte: str) -> str {
    let ctx = CtxSegment::new(4096);   // managed allocation
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

#### Benefits

| Feature | Advantage |
|---------|-----------|
| Zero new keywords | Only a type `CtxGuard` and its `new()` method |
| Natural syntax | Uses existing `{ }` blocks to delimit scope |
| Safety | Rollback guaranteed even on error or early return |
| Composability | Guard can be passed to functions, stored, or nested |
| Familiarity | Rust/C++ developers will recognize RAII pattern |

#### Interaction with Other Features

| Feature | Interaction | Result |
|---------|-------------|--------|
| `CtxView` | Guard protects segment while immutable views active | Views remain valid; underlying segment restored |
| `ctx_share` | Guard owns restoration; shared handles see rolled-back state | Predictable semantics |
| Context Versioning | Guard provides lightweight rollback without full versioning | Complementary: versioning for history, guard for scoping |
| Copy-on-Write | Guard restores pre-CoW state | CoW duplications within guard scope are discarded |

### Design Vision: Copy-on-Write

```la
let shared = ctx_alloc(4096);
ctx_append(shared, "Team briefing: ");

let agent_a = ctx_share(shared);
let agent_b = ctx_share(shared);

// Both see "Team briefing: "
ctx_append(agent_a, "Agent A's notes");
// Triggers CoW — agent_a now has its own copy
// agent_b and shared still see only "Team briefing: "
```

---

## Error Reference

| Error | Trigger | Recovery |
|-------|---------|----------|
| `HeapError::Overflow { requested, available }` | `ctx_alloc(N)` where N > available | Fatal — increase `--context` or free segments |
| `HeapError::InvalidHandle(id)` | Use of freed or never-allocated segment ID | Fatal — fix the program logic |

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) — Token Heap section and VM integration
- [SPEC.md §7](SPEC.md#7-context-management) — Context primitive specifications
- [ROADMAP.md](ROADMAP.md) — Phase 10–12 planned Token Heap extensions
