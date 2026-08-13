# AI Quality Standard

This document defines what useful AI-generated tests, documentation, and
benchmarks look like in Frogbyte.

It is shared by the Claude generation agent and Codex review/fallback work.
Task permissions and writable paths are defined separately in `CLAUDE.md` and
`AGENTS.md`.

## Core method

For every quality task:

1. Understand the pull request intent, linked issue, changed behavior, and
   relevant implementation.
2. Inspect the existing tests, documentation, benchmarks, and relevant review
   feedback before editing.
3. Identify concrete gaps. Do not generate artifacts merely because a task was
   requested.
4. Make the smallest set of changes that materially improves quality.
5. Review the complete generated diff adversarially before finishing.
6. Remove weak, redundant, misleading, speculative, or unnecessary work.

Discovering that an existing artifact is misleading or methodologically invalid
is not resolved by documenting the limitation.

When the allowed scope permits it, correct the artifact or stop presenting it
as valid or directly comparable before adding new coverage.

Producing no change is a valid result when the existing artifacts are already
sufficient or the allowed scope cannot improve them safely.

## Tests

Tests exist to detect regressions, not to maximize test count.

A useful test should target a distinct observable behavior, contract, invariant,
boundary, transition, or realistic failure mode introduced or affected by the
pull request.

Before keeping a test, be able to answer:

> What realistic incorrect implementation should make this test fail?

Prefer tests that:

- exercise behavior through the public or intended API;
- isolate the property being tested with deliberate inputs;
- cover important invalid, boundary, and repeated-transition behavior;
- exercise ownership, destruction, aliasing, or state invariants when relevant;
- complement existing coverage instead of repeating the same signal.

Avoid tests that:

- differ from an existing test only by arbitrary values;
- assert incidental implementation details without a contract reason;
- duplicate another test's effective failure signal;
- contain assertions too weak to detect the targeted regression.

Existing generated tests may be consolidated or removed when they are stale,
misleading, or strictly redundant, provided meaningful behavioral coverage is
not reduced.

## Documentation

Documentation exists to preserve understanding of contracts and non-obvious
reasoning.

Prioritize:

- public API contracts and observable behavior;
- invariants and invalid-state behavior;
- ownership, lifetime, and aliasing requirements;
- safety requirements;
- non-obvious algorithmic or architectural rationale.

Avoid narrating obvious code or promoting incidental implementation details to
stable guarantees.

`SAFETY[UNSAFE-XXX]` comments must explain why the associated unsafe operation
is sound: the relevant invariant, precondition, ownership rule, alignment,
initialization, lifetime, or aliasing fact. They may be added, corrected,
relocated, or removed when the code or its safety reasoning changes.

When the same unsafe boundary remains, preserve its stable Safety Review
identifier unless there is a concrete reason to replace it.

## Benchmarks

Benchmarks exist to answer meaningful performance questions and provide stable
long-term baselines.

Before keeping a benchmark, be able to state:

> What performance question does this benchmark answer?

Prefer workloads that represent performance-sensitive operations, scaling,
allocation behavior, memory access, cache behavior, or repeated hot-path work.

For every benchmark, make the measurement boundary deliberate:

- identify the logical starting state;
- identify the operation or workload being measured;
- decide which setup and teardown costs belong inside or outside measurement;
- choose representative input scales;
- preserve stable benchmark names when workload semantics remain stable.

For direct comparisons, both sides must perform equivalent logical work from
equivalent logical states. Setup must not hide a meaningful cost for only one
side. If workloads are materially different, do not present them as a direct
comparison.

Use Criterion iteration, batching, throughput, parameterization, and
`std::hint::black_box` only when they make the intended measurement more valid
or interpretable.

Avoid trivial cold-path microbenchmarks, unsupported performance claims, hard
CI timing thresholds, and benchmarks for future behavior that does not exist.

## Final review

Before finishing any generation task, inspect the entire generated diff as if
reviewing someone else's work.

Keep only artifacts with a clear purpose and strong signal. Prefer a smaller,
higher-quality change over broad low-value coverage.
