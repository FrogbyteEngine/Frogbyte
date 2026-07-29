### Unsafe boundary

**Purpose**

What operation or abstraction requires `unsafe`?

**Why `unsafe` is required**

Why is a reasonable safe Rust implementation insufficient?

**Safety contract**

Describe:

- caller obligations;
- maintained invariants;
- initialization and aliasing assumptions;
- relevant thread-safety assumptions.

**Invalidation**

Which pointers, references, handles, or indices may become invalid, and when?

**Validation**

List the relevant tests, Miri runs, fuzzing, or manual review evidence.