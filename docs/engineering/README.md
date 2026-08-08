# Engineering Policies

This directory contains the policies used to keep Frogbyte understandable, testable, auditable, and reproducible while the engine evolves.

Policies define shared rules and review expectations. They do not replace technical design documents, implementation notes, or milestone acceptance criteria.

## Policy index

| Document | Purpose |
|---|---|
| [`TESTING_STRATEGY.md`](TESTING_STRATEGY.md) | Test layers, reference-model testing, property tests, Miri, fuzzing, and regression preservation |
| [`UNSAFE_POLICY.md`](UNSAFE_POLICY.md) | Conditions for introducing, documenting, reviewing, and validating project-authored `unsafe` code |
| [`UNSAFE_REGISTRY.md`](UNSAFE_REGISTRY.md) | Inventory of unsafe abstractions and their detailed safety records |
| [`unsafe/`](unsafe/) | One detailed record per unsafe abstraction or safety boundary |