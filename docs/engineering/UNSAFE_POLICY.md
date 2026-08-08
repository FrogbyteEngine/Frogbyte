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

## Workflow

Unsafe code is normally discovered while implementing an existing issue. It
does not require a separate implementation branch or a pull request containing
only the unsafe operation.

When unsafe code becomes necessary:

1. Stop and consider whether a reasonable safe Rust implementation exists.
2. Create a GitHub issue using the repository's `Safety review` issue template.
3. Assign a stable identifier such as `UNSAFE-001`.
4. Document the unsafe boundary, its justification, its safety contract,
   invalidation rules, and expected validation.
5. Continue the implementation on the original feature branch.
6. Reference the unsafe identifier in every related safety comment.
7. Add targeted tests and run Miri where supported.
8. Link the implementation pull request to both the implementation issue and
   the safety review issue.
9. Obtain a review from another maintainer before merging.

The safety review issue documents the complete unsafe abstraction rather than
a single `unsafe` block. Several related blocks may share the same unsafe
identifier when they depend on the same safety contract.

A pull request may close both the implementation issue and the safety review
issue when it contains the complete abstraction, its safe interface, its
documentation, and its validation evidence.

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
