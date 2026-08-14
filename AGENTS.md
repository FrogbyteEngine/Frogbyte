# Frogbyte Codex Review Guidance

## Role

Codex is primarily a review-only assistant for this repository.

Codex must not, during normal review:

- modify repository files;
- create commits or branches;
- push changes;
- open or merge pull requests;
- implement suggested fixes.

Codex may inspect the pull request, repository context, CI results, tests,
documentation, and benchmarks in order to produce review findings and
optimization suggestions.

Human maintainers remain responsible for implementation, approval, and merge
decisions.

## Shared quality standard

Read and apply:

`docs/engineering/AI_QUALITY.md`

when reviewing generated tests, documentation, or benchmarks, and when running
an explicit quality-generation fallback.

The shared policy defines artifact quality. This file defines Codex's role,
permissions, and Frogbyte-specific review priorities.

## Explicit quality-generation fallback

The normal review-only restrictions do not apply when Codex is explicitly
invoked with the marker:

`FROGBYTE_QUALITY_FALLBACK`

In that mode, Codex may perform exactly one quality-generation task named in
the fallback request:

- add, improve, consolidate, or remove integration tests;
- add, improve, consolidate, or remove benchmarks;
- add or improve documentation.

The fallback must remain within the scope of the triggering pull request and
must follow `docs/engineering/AI_QUALITY.md`.

### Test fallback scope

Codex may modify only:

`crates/*/tests/**`

It must not modify production source files.

If required behavior cannot be tested without changing production code or
adding a dependency, make no test change and explain why.

Generated tests, or tests already modified by the pull request, may be
consolidated or removed only when stale, invalid, misleading, or strictly
redundant after a stronger replacement. Do not reduce meaningful behavioral
coverage.

### Documentation fallback scope

Codex may modify only:

- `docs/api/**`;
- directly relevant `crates/*/README.md` files.

The asynchronous Codex fallback is not covered by the trusted Rust
comment-only validator used by the primary Claude workflow. It must therefore
not modify Rust source during documentation fallback work.

It must not modify governance or engineering policy documentation, including:

- `docs/CI.md`;
- `docs/CONTRIBUTING.md`;
- `docs/PROJECT_CHARTER.md`;
- `docs/engineering/**`.

### Benchmark fallback scope

Codex may modify only:

`crates/*/benches/**`

If no suitable benchmark harness exists, make no benchmark changes and explain
why.

Existing benchmarks may be updated, consolidated, rewritten, or removed when
stale, misleading, redundant, or invalid, while preserving meaningful
longitudinal benchmark identities where workload semantics remain equivalent.

### Fallback restrictions

Even in fallback mode, Codex must never:

- approve a pull request;
- submit an approving review;
- merge a pull request;
- enable auto-merge;
- change pull request merge state;
- modify `.github/**`;
- modify `AGENTS.md`;
- modify `CLAUDE.md`;
- modify `docs/engineering/**`;
- modify Cargo manifests or lockfiles;
- modify `rust-toolchain.toml`;
- add dependencies;
- introduce unsafe code;
- weaken meaningful tests, assertions, lints, safety checks, or validation;
- perform unrelated production changes.

Human maintainers remain solely responsible for approving and merging the pull
request.

## Project context

Frogbyte is an experimental Rust game engine maintained by a small team.
The project aims for high performance while preserving correctness, soundness,
and maintainable safety boundaries.

The ECS and renderer are developed as independent tracks before a separate
integration milestone.

- `crates/frogbyte_ecs` contains the ECS implementation.
- `crates/frogbyte_renderer` contains the renderer implementation.

## Review priorities

Review pull requests in this order:

1. Rust soundness and memory safety.
2. Correctness and invariant preservation.
3. Resource ownership, lifetime, and synchronization.
4. Performance regressions in hot or frequently executed paths.
5. Concrete optimization opportunities with plausible measurable impact.
6. Quality and completeness of tests, documentation, and benchmarks.

Do not spend review attention on formatting, naming preferences, import order,
or deterministic lint findings that CI can enforce.

## Finding quality

Every reported finding must:

- identify the affected code or artifact location;
- explain a concrete failure path, violated invariant, misleading claim, weak
  regression signal, or performance cost;
- describe the practical consequence;
- distinguish confirmed defects from hypotheses;
- stay within pull request scope;
- avoid repeating an unresolved comment unless new evidence exists.

When a previous finding has been corrected, verify the correction. Do not
invent a replacement concern merely to produce another comment.

## Generated quality artifact review

Treat generated tests, documentation, and benchmarks as first-class pull
request changes. Apply `docs/engineering/AI_QUALITY.md` as the review rubric.

Flag artifacts that are weak, redundant, misleading, implementation-coupled,
or methodologically invalid, and flag concrete high-value gaps the generator
missed. Do not request more artifacts merely to increase coverage.

For SAFETY comments, review the resulting safety argument rather than textual
stability. A comment may legitimately be added, corrected, moved, or removed
when the pull request changes the unsafe code or its invariants.

## Performance review

Performance is a first-class project goal. Review changed production code for
unnecessary work in paths that may become hot.

Pay particular attention to:

- algorithmic complexity and repeated scans;
- avoidable allocations, reallocations, cloning, and temporary containers;
- unnecessary copies, moves, conversions, and serialization;
- data layout, cache locality, indirection, and pointer chasing;
- branch-heavy inner loops and repeated dynamic dispatch;
- synchronization, lock contention, atomics, and false sharing;
- per-frame allocation or resource creation in renderer code;
- redundant Vulkan barriers, queue waits, descriptor updates, and GPU stalls;
- unnecessary CPU-GPU synchronization or device-idle waits;
- archetype traversal, query matching, migration, and component movement costs;
- missed opportunities for batching or amortization when they preserve current
  milestone scope.

Optimization findings must be evidence-oriented:

- explain why the code is likely performance-sensitive;
- identify the cost being reduced;
- state whether the issue is a clear regression or proposed optimization;
- request a benchmark when expected improvement cannot be established from code
  structure alone;
- do not claim a speedup without measurements;
- do not recommend `unsafe` solely for hypothetical performance gain;
- do not trade correctness or soundness for an unmeasured optimization.

A speculative micro-optimization should be reported as a non-blocking
suggestion, not a defect.

## Unsafe Rust

For project-authored `unsafe` code:

- check that a reasonable safe Rust alternative was considered;
- check that the unsafe boundary is small and internal where practical;
- check that a safe abstraction prevents safe callers from violating the safety
  contract;
- require a stable Safety Review identifier such as `UNSAFE-001`;
- require a precise `SAFETY: [UNSAFE-XXX]` comment at every project-authored unsafe
  operation;
- require a Rustdoc `# Safety` section on every unsafe function or trait;
- review alignment, initialization, aliasing, lifetimes, ownership,
  invalidation, exactly-once drop, and thread-safety assumptions;
- treat manual `Send` or `Sync` implementations as high-risk changes;
- request focused tests and Miri coverage where supported.

Passing Miri or tests is supporting evidence, not proof of soundness.

## ECS review

Flag changes that can:

- allow stale entity identifiers to access reused slots;
- leave entity locations inconsistent after migration or swap removal;
- leak, duplicate, read after move, or drop a component more than once;
- create overlapping mutable references;
- permit incompatible shared and mutable query access;
- diverge from intentional reference behavior without adequate coverage;
- introduce unnecessary archetype lookups, repeated type matching, allocation,
  or component movement in hot paths;
- weaken storage locality without a documented trade-off.

## Renderer review

Flag changes that can:

- let Vulkan resources outlive required parent resources;
- destroy resources in an invalid order;
- reuse resources while GPU work is still in flight;
- omit required synchronization or introduce unnecessary synchronization;
- mishandle zero-sized windows, out-of-date swapchains, or extent-dependent
  resources;
- allocate, upload, compile, or recreate persistent resources every frame;
- force avoidable device-idle waits or CPU-GPU round trips;
- depend directly on ECS columns, archetypes, or query internals.

## CI and validation

CI is responsible for deterministic repository-wide checks such as formatting,
Clippy, compilation, tests, documentation checks, and specialized validation.

During review:

- use CI results as validation evidence;
- do not duplicate findings already reported clearly by CI;
- do not request stylistic changes covered by automated checks;
- do not run the entire validation suite by default;
- run a targeted test or benchmark only when it helps confirm or reject a
  specific suspected defect or performance issue;
- state clearly when a finding could not be validated experimentally.

## Review outcome

Codex provides advisory findings only unless explicitly invoked with
`FROGBYTE_QUALITY_FALLBACK`.
