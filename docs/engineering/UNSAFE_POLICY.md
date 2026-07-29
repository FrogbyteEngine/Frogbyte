# Unsafe Code Policy

## Purpose

`unsafe` code is permitted only where it is necessary to implement low-level storage, type erasure, graphics interop, or a measured performance requirement that cannot be expressed adequately with safe Rust.

The public API must remain safe by default.

## Rules

Every introduction or modification of `unsafe` code must:

- remain inside a limited internal module or crate;
- expose a safe abstraction whenever possible;
- include an English `// SAFETY:` comment at the operation site;
- document caller obligations and maintained invariants;
- document pointer, reference, and handle invalidation;
- document aliasing and initialization assumptions;
- avoid manual `Send` or `Sync` implementations without a dedicated review;
- have targeted automated tests;
- run under Miri where the code path is supported;
- be reviewed for simpler safe alternatives.

## Review template

```markdown
### Unsafe primitive

**Purpose**

**Why safe Rust is insufficient**

**Required preconditions**

**Maintained invariants**

**Initialization assumptions**

**Aliasing assumptions**

**Invalidated pointers, references, or handles**

**Thread-safety assumptions**

**Tests and tools covering the primitive**
```

## Safety comments

A useful safety comment explains why the required conditions hold at that exact operation.

```rust
// SAFETY: `row` is strictly less than the initialized column length, the
// pointer was allocated for values of `T`, and no mutable reference to the
// same element exists for the duration of the returned borrow.
let value = unsafe { &*column_ptr.add(row).cast::<T>() };
```

A comment such as `// SAFETY: this is safe` is not acceptable.

## Validation limits

Miri, sanitizers, tests, and fuzzing can detect classes of defects. Passing these tools does not by itself prove soundness. The primary safety argument remains the documented invariant and the proof that safe callers cannot violate it.
