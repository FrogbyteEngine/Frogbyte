# Continuous Integration

Frogbyte uses GitHub Actions to keep the repository reproducible, portable, and safe to change.

The automation is split into several layers:

- **Required CI** validates code quality and build correctness on every pull request and push to `main`.
- **Security validation** checks dependency changes and GitHub Actions configuration.
- **Miri** performs specialized undefined-behavior checks on the ECS.
- **AI Quality** provides opt-in generation of tests, documentation, and benchmarks with trusted scope validation.
- **Pull request policy** enforces repository contribution conventions.
- **Scheduled validation** detects future Rust or dependency compatibility problems before they affect development.
- **Dependabot** keeps Rust dependencies and GitHub Actions up to date.
- **Project automation** synchronizes linked issue status with the pull request lifecycle.

## Required CI

Workflow: `.github/workflows/ci.yml`

The main CI workflow runs:

- on pull requests targeting `main`;
- on pushes to `main`;
- when manually triggered with `workflow_dispatch`.

For pull requests, an older CI run is cancelled when a newer commit is pushed to the same pull request. This avoids wasting runner time on obsolete revisions.

The workflow uses the Rust toolchain defined in `rust-toolchain.toml`.

### Formatting

Runs on Linux:

```shell
cargo fmt --all -- --check
```

This verifies that every Rust source file follows the formatting produced by `rustfmt`.

The check does not modify files. Run `cargo fmt --all` locally to automatically fix formatting differences.

### Clippy

Runs on both:

- Ubuntu 24.04;
- Windows Server 2025.

```shell
cargo clippy \
    --workspace \
    --all-targets \
    --all-features \
    --locked \
    -- \
    -D warnings
```

Clippy performs static analysis over the complete workspace.

`-D warnings` promotes every warning to an error, preventing new warnings from being introduced into `main`.

Running Clippy on both Linux and Windows also helps detect platform-specific compilation or linting differences.

### Tests

Runs on both Linux and Windows:

```shell
cargo test \
    --workspace \
    --all-targets \
    --all-features \
    --locked \
    --no-fail-fast
```

This executes the complete workspace test suite.

`--no-fail-fast` allows Cargo to continue running independent test targets after a failure so that CI can report as much information as possible in a single run.

### Documentation tests

Runs on Linux:

```shell
cargo test \
    --workspace \
    --doc \
    --all-features \
    --locked \
    --no-fail-fast
```

Documentation examples are executable Rust code.

Doctests make sure code examples in API documentation continue to compile and behave correctly as the implementation evolves.

### Rustdoc

Runs on Linux with:

```text
RUSTDOCFLAGS="-D warnings"
```

and:

```shell
cargo doc \
    --workspace \
    --all-features \
    --no-deps \
    --locked
```

This verifies that the complete public API documentation can be generated without warnings.

`--no-deps` limits documentation generation to Frogbyte crates instead of rebuilding documentation for external dependencies.

### Release build

Runs on Windows:

```shell
cargo build \
    --workspace \
    --all-targets \
    --all-features \
    --release \
    --locked
```

Tests and debug builds are not enough to guarantee that release compilation works.

This job makes sure the complete workspace can also be built using release optimizations on the primary Windows target.

### Required CI gate

The `CI / Required` job depends on every main CI job:

- Formatting;
- Clippy;
- Tests;
- Documentation tests;
- Rustdoc;
- Release build.

It succeeds only when all of them completed successfully.

This provides a single stable status check that can be used as a branch-protection merge requirement while individual jobs remain free to evolve.

## Security validation

Workflow: `.github/workflows/security.yml`

Security validation runs:

- on pull requests targeting `main`;
- on pushes to `main`;
- when manually triggered.

It covers dependency changes and the security of GitHub Actions themselves.

### Dependency review

Dependency review runs only on pull requests.

It uses GitHub's dependency graph to compare dependencies before and after the pull request.

The workflow rejects dependency changes that introduce a known vulnerability with a severity of:

- moderate;
- high;
- critical.

This protects the repository against accidentally introducing a known vulnerable dependency through `Cargo.toml` or `Cargo.lock`.

The dependency graph must remain enabled in the repository settings for this check to work.

### GitHub Actions security

Frogbyte uses `zizmor` to audit the repository's GitHub Actions configuration.

The analysis looks for workflow security problems such as:

- mutable third-party action references;
- overly broad permissions;
- unsafe credential persistence;
- dangerous GitHub App token usage;
- insecure workflow expressions or configuration.

Results are uploaded to GitHub Code Scanning as SARIF so findings can be inspected directly from the repository security interface.

A successful `zizmor` job means the analyzer itself completed successfully. Findings may still be reported through Code Scanning depending on their severity and the analyzer configuration.

### Required security gate

`Security / Required` aggregates the security jobs into one stable status.

On pull requests:

- dependency review must succeed;
- the GitHub Actions security analysis must succeed.

On events where dependency review does not apply, its skipped state is accepted.

## GitHub Actions supply-chain security

External GitHub Actions execute code inside CI runners and are therefore part of Frogbyte's software supply chain.

Actions should be pinned to immutable commit SHAs instead of mutable version tags.

Prefer:

```yaml
uses: actions/checkout@<full-commit-sha> # vX.Y.Z
```

instead of:

```yaml
uses: actions/checkout@vX
```

The commit SHA guarantees that CI executes exactly the reviewed version of an action.

The version comment remains for readability while Dependabot can propose future SHA updates through normal pull requests.

Checkout steps should also use:

```yaml
with:
  persist-credentials: false
```

unless Git credentials are explicitly required later in the job.

GitHub tokens and GitHub App tokens should follow the principle of least privilege and receive only the permissions needed by their workflow.

## Miri

Workflow: `.github/workflows/miri.yml`

Miri performs specialized validation for the ECS crate.

It runs when a pull request or push affecting `main` changes:

- `crates/frogbyte_ecs/**`;
- `Cargo.toml`;
- `Cargo.lock`;
- the Miri workflow itself.

It can also be triggered manually.

Frogbyte pins a dedicated nightly toolchain for Miri:

```text
nightly-2026-07-30
```

The workflow installs Miri, prepares its sysroot, and executes:

```shell
cargo +"nightly-2026-07-30" miri test -p frogbyte_ecs
```

Miri interprets Rust code while checking operations that can result in undefined behavior.

This is particularly useful for the ECS because low-level memory manipulation and future unsafe optimizations require stronger validation than normal unit tests can provide.

Miri complements the normal test suite; it does not replace it.

## AI Quality

Workflow: `.github/workflows/ai_quality.yml`

AI Quality is an opt-in pull request automation used to generate or maintain quality artifacts after implementation work exists.

It is triggered by applying one of these labels:

- `agent:tests`;
- `agent:docs`;
- `agent:benchmarks`.

Only one AI Quality run may modify a pull request branch at a time. Additional label-triggered runs for the same pull request remain queued so tests, documentation, and benchmarks can be requested together without racing each other.

### Shared quality policy

Generated-artifact quality is defined in:

```text
docs/engineering/AI_QUALITY_POLICY.md
```

Claude-specific permissions and task scopes are defined in:

```text
CLAUDE.md
```

Codex review and fallback permissions are defined in:

```text
AGENTS.md
```

The shared policy favors high-signal artifacts over quantity. Generation begins by identifying concrete gaps in tests, documentation, or benchmarks and ends with an adversarial self-review that removes weak, redundant, misleading, or speculative output.

The policy is intentionally general rather than being tied to a particular ECS type, pull request, or previously observed generation mistake.

### Trusted policy source

AI Quality uses `pull_request_target` because the workflow requires credentials.

The workspace root is checked out from the trusted base branch. `CLAUDE.md`, `AGENTS.md`, the shared quality policy, the trusted validator, and the Rust token guard therefore come from the trusted base commit rather than the pull request head.

The pull request head is checked out separately under:

```text
pr-head/
```

and is treated as untrusted data.

A pull request that changes `CLAUDE.md`, `AGENTS.md`, `docs/engineering/AI_QUALITY_POLICY.md`, or the AI Quality workflow does not change the trusted instructions used by its own already-running `pull_request_target` job. Those changes become active for later AI Quality runs after they are merged into the trusted base branch.

The completion comment records the trusted base commit used as the quality-policy source so this behavior remains visible during debugging.

### Claude generation

Claude is the primary generation agent.

All three tasks use a maximum of:

```text
40 turns
```

The benchmark task continues to use the configured higher-reasoning model settings while tests and documentation use the normal model selection.

Claude must read the trusted `CLAUDE.md` and shared quality policy before editing.

The privileged job never executes pull-request-controlled project code. Claude must not run Cargo, tests, benchmarks, formatters, linters, build scripts, proc macros, project binaries, or other commands derived from pull request contents.

Existing GitHub CI remains the deterministic execution and validation authority after generated changes are published.

### Test generation scope

`agent:tests` may modify only:

```text
crates/*/tests/**
```

The agent may add, improve, consolidate, or remove integration tests when doing so improves signal without reducing meaningful behavioral coverage.

Tests are evaluated by regression value rather than test count. Each retained generated test should target a distinct useful contract, invariant, boundary, state transition, or realistic failure path.

### Documentation generation scope

`agent:docs` may modify:

- Rust source files already changed by the pull request under `crates/*/src/**/*.rs`, but only comments and whitespace;
- directly relevant touched-crate `README.md` files;
- `docs/api/**`.

It may not modify governance or engineering policy documentation.

The trusted Rust token guard compares source before and after documentation generation. Every non-comment Rust token kind and exact lexeme must remain unchanged, in the same order, with the same lexical separation from adjacent code tokens.

Comments are maintainable documentation. This includes non-doc `SAFETY` comments, which may be added, rewritten, moved, or removed when required to keep the safety explanation correct after the pull request changes code.

The token guard does not attempt to prove that a SAFETY explanation is semantically correct. It proves only that executable Rust tokens were not changed. The shared policy, Claude self-review, optional Codex review, normal CI, and human review remain responsible for the quality of the safety reasoning.

Rustdoc can still affect source locations and is visible to macros as `doc` attributes. These effects remain outside the mechanical token-integrity guarantee.

### Benchmark generation scope

`agent:benchmarks` may modify only:

```text
crates/*/benches/**
```

The agent may maintain existing benchmarks instead of always appending new ones.

Benchmarks must begin with a meaningful performance question. Direct comparisons must perform equivalent logical work from equivalent logical starting states, and setup or teardown must not be hidden for only one side of a comparison.

Criterion iteration method, batch size, throughput, representative input scales, and benchmark identity must be chosen deliberately according to the shared quality policy.

### Trusted audit and publish

Claude does not own Git history or repository publication.

After generation, a trusted workflow step:

1. verifies the trusted validator hash;
2. validates that every generated path belongs to the task scope;
3. runs the trusted Rust token guard for Rust documentation changes;
4. verifies that the pull request head did not change while generation was running;
5. creates one deterministic commit;
6. pushes that commit to the existing pull request branch.

If the pull request head changes during generation, publication fails closed rather than overwriting newer work.

### Conditional Codex quality review

Claude returns a structured `codex_review_recommended` boolean after its adversarial self-review.

A read-only Codex second pass is requested only when independent judgment is likely to add meaningful value, such as:

- SAFETY comment maintenance;
- a direct comparative benchmark;
- a non-obvious benchmark measurement boundary;
- generated tests based on ambiguous or underspecified behavior;
- unresolved uncertainty after self-review.

The trusted workflow also detects changed `SAFETY` lines mechanically and requests a Codex quality review even if Claude forgot to recommend one.

The review handoff uses:

```text
FROGBYTE_QUALITY_REVIEW
```

This mode is advisory and read-only. Codex reviews the generated commit against `AGENTS.md` and the shared quality policy but must not modify files, push commits, approve, merge, enable auto-merge, or change pull request state.

A failed optional Codex quality-review handoff does not invalidate an otherwise successfully published Claude generation. Normal CI and human review can continue.

### Claude quota fallback

Codex generation fallback remains separate from the optional read-only quality review.

Fallback is triggered only when the workflow confidently classifies Claude's failure as included-usage quota exhaustion.

A normal Claude error or configured turn-limit failure does not trigger generation fallback.

The fallback handoff uses:

```text
FROGBYTE_QUALITY_FALLBACK
```

In fallback mode Codex may write only within the task-specific scope defined in `AGENTS.md`.

The asynchronous documentation fallback may not modify Rust source because it does not pass through the trusted local Rust token guard.

### Validation after generation

The privileged AI job does not claim that local project validation passed.

After a generated commit is pushed, normal GitHub Actions validate the resulting pull request revision.

The AI Quality completion comment reports:

- files changed;
- local validation as not run in the privileged AI job;
- CI validation as pending after push or not applicable;
- whether an advisory Codex quality review was requested;
- the trusted base commit used as the quality-policy source.

AI-generated quality artifacts do not replace human review and do not grant merge authority to either Claude or Codex.

## Pull request policy

Workflow: `.github/workflows/validate_pull_request.yml`

Every pull request must follow the Frogbyte contribution policy.

The expected title format is:

```text
<Area>: <Imperative description>
```

Supported areas are:

- `ECS`;
- `Renderer`;
- `Integration`;
- `Infrastructure`;
- `Docs`;
- `Architecture`;
- `Dependencies`;
- `CI`;
- `Safety`.

For example:

```text
Renderer: Recreate swapchain resources after resize
```

Pull request titles:

- must match the expected area format;
- must not exceed 72 characters;
- must not end with a period;
- must not contain `WIP`.

GitHub's draft state should be used instead of adding `WIP` to the title.

Human-authored pull request bodies must also reference an issue with one of the supported forms:

```text
Closes #123
Fixes #123
Resolves #123
Refs #123
```

Repository-qualified issue references are also supported.

Dependabot dependency-update pull requests authored by `dependabot[bot]` and using the `Dependencies` area are exempt from the issue-reference requirement.

The exception applies only to the issue reference. Dependabot pull requests must still satisfy the normal title policy and all required CI and security validation.

Human-authored dependency pull requests must continue to reference an issue.

This policy keeps the pull request history consistent and preserves traceability between normal development changes and their related issues without requiring artificial issues for automated dependency maintenance.

## Scheduled validation

Workflow: `.github/workflows/scheduled.yml`

Some compatibility problems cannot be detected by testing only the currently pinned environment.

A scheduled workflow therefore runs every Monday at `04:17 UTC` and can also be started manually.

Scheduled validation is preventive and is not intended to replace the required pull request CI.

### Future Rust compatibility

The workspace test suite runs against:

- Rust beta;
- Rust nightly.

```shell
cargo +<toolchain> test \
    --workspace \
    --all-targets \
    --all-features \
    --locked \
    --no-fail-fast
```

This provides early warning when an upcoming Rust release introduces a compiler, lint, or compatibility change that affects Frogbyte.

The normal CI continues to use the pinned stable toolchain.

### Latest compatible dependencies

The scheduled workflow also executes:

```shell
cargo update
```

before running the test suite.

This intentionally ignores the currently committed dependency resolution and asks Cargo to resolve the latest versions allowed by the manifests.

It helps detect situations where:

- `Cargo.lock` currently works;
- the dependency version requirements are valid;
- but a newly released compatible dependency breaks the project.

Because this is an exploratory compatibility check, the updated lock file is not committed by the workflow.

## Dependabot

Configuration: `.github/dependabot.yml`

Dependabot maintains two dependency ecosystems.

### Rust dependencies

Cargo dependencies are checked every Monday at `06:00` in the `Europe/Paris` timezone.

### GitHub Actions

GitHub Actions dependencies are checked every Monday at `06:30` in the `Europe/Paris` timezone.

This is especially important because GitHub Actions are pinned to immutable commit SHAs.

Dependabot can propose a pull request that moves a pinned action from one reviewed commit SHA to another instead of silently changing the action behind a mutable tag.

### Pull request limits

Dependabot can keep at most five open update pull requests per ecosystem.

Dependabot commits use the `Dependencies` prefix so they comply with Frogbyte's pull request naming conventions.

Dependabot-authored dependency update pull requests are exempt from the normal issue-reference requirement because they are generated maintenance changes rather than implementation work originating from a Frogbyte issue.

All other pull request policy, CI, and security requirements continue to apply.

## Toolchain and reproducibility

The repository pins its stable Rust environment in:

```text
rust-toolchain.toml
```

The current toolchain is:

```text
Rust 1.97.1
```

with the minimal profile plus:

- `clippy`;
- `rustfmt`.

Using a repository-level toolchain file means developers and CI use the same Rust release automatically when using `rustup`.

`Cargo.lock` is committed to the repository.

Required CI commands use:

```text
--locked
```

This prevents Cargo from silently changing dependency resolution during validation.

Together, the pinned Rust toolchain, committed lock file, and pinned GitHub Action SHAs make CI runs substantially more reproducible.

## Project status automation

Workflow: `.github/workflows/sync_project_review_status.yml`

The project automation keeps the linked issue status synchronized with the pull request lifecycle.

When a pull request becomes ready for review, its linked issue is moved to:

```text
In review
```

When the pull request is converted back to a draft, the linked issue is moved to:

```text
In progress
```

The workflow uses a GitHub App token with explicit least-privilege permissions for:

- organization projects;
- pull requests;
- issues.

Repository secrets are not exposed to pull requests from forks, so this automation only runs when the pull request branch belongs to the Frogbyte repository.

This workflow does not validate code, but it is part of the repository automation surrounding the review process.

## Local validation

Before marking a pull request ready for review, run the main CI checks locally.

```shell
cargo fmt --all -- --check

cargo clippy \
    --workspace \
    --all-targets \
    --all-features \
    --locked \
    -- \
    -D warnings

cargo test \
    --workspace \
    --all-targets \
    --all-features \
    --locked \
    --no-fail-fast

cargo test \
    --workspace \
    --doc \
    --all-features \
    --locked \
    --no-fail-fast

cargo build \
    --workspace \
    --all-targets \
    --all-features \
    --release \
    --locked
```

Validate Rust documentation on Bash-compatible shells with:

```shell
RUSTDOCFLAGS="-D warnings" \
cargo doc \
    --workspace \
    --all-features \
    --no-deps \
    --locked
```

On PowerShell:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --all-features --no-deps --locked
Remove-Item Env:RUSTDOCFLAGS
```

When modifying the ECS or its low-level memory behavior, also run Miri:

```shell
rustup toolchain install nightly-2026-07-30 --component miri
cargo +"nightly-2026-07-30" miri setup
cargo +"nightly-2026-07-30" miri test -p frogbyte_ecs
```

Local validation reduces CI feedback time, but GitHub Actions remains the authoritative validation environment because it also verifies the project on the configured Linux and Windows runners.

## Merge expectations

Before merging a pull request:

1. The pull request policy must pass.
2. Required CI must pass.
3. Required security validation must pass.
4. Specialized checks such as Miri must pass when they apply.
5. Relevant Code Scanning findings should be reviewed.
6. The pull request should no longer be a draft.

Scheduled compatibility checks and Dependabot are preventive maintenance mechanisms and are not substitutes for pull request validation.
