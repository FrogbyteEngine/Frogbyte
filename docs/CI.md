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

AI Quality is an opt-in pull request workflow for generating and maintaining
quality artifacts after implementation work exists.

It is triggered by these labels:

- `agent:tests`;
- `agent:docs`;
- `agent:benchmarks`.

Only one AI Quality run may modify a pull request branch at a time. Additional
label-triggered runs for the same pull request are queued.

### Responsibilities

The workflow deliberately separates quality rules from permissions and
mechanical validation:

- `docs/engineering/AI_QUALITY.md` defines what useful tests, documentation, and
  benchmarks look like;
- `CLAUDE.md` defines Claude's writable scopes and privileged-workflow rules;
- `AGENTS.md` defines Codex review behavior and quota-fallback permissions;
- `.github/scripts/validate_ai_quality.py` validates task path scope;
- `.github/tools/ai_quality_token_guard/` validates comment-only Rust source
  edits for `agent:docs`.

This keeps artifact-quality guidance in one shared policy instead of repeating
it in the workflow prompt.

### Trusted execution model

AI Quality uses `pull_request_target` because the workflow requires credentials.
The workspace root is therefore checked out from the trusted base branch.

The pull request head is checked out separately under:

```text
pr-head/
```

and is treated as untrusted data. Pull-request-controlled project code is never
executed in the privileged job.

Claude may inspect and edit the isolated snapshot within the selected task
scope, but must not run Cargo, tests, benchmarks, formatters, linters, build
scripts, proc macros, project binaries, or commands derived from pull request
contents.

The trusted `CLAUDE.md`, shared AI quality policy, validator, and token guard
come from the base commit. A pull request changing those files does not change
the trusted rules used by its own run; the new rules become active after merge.

### Claude generation

Claude is the normal quality-generation agent. Every task has a maximum budget
of:

```text
40 turns
```

Before editing, Claude inspects the pull request intent, linked issue, changed
implementation, existing relevant quality artifacts, and relevant review
feedback. It then follows the shared quality policy and performs an adversarial
self-review before finishing.

Producing no change is valid when no useful in-scope improvement exists.

Task scopes are:

```text
agent:tests       crates/*/tests/**
agent:benchmarks  crates/*/benches/**
```

`agent:docs` may maintain comments and Rustdoc in Rust source files already
changed by the pull request, directly relevant touched-crate `README.md` files,
and `docs/api/**`.

Rust documentation edits may add, rewrite, move, or remove comments, including
`SAFETY: [UNSAFE-XXX]` comments. The token guard requires all non-comment Rust
tokens and their lexical separation to remain unchanged.

The guard proves source-token integrity, not that documentation or safety
reasoning is semantically correct. Normal review remains required.

### Trusted audit and publication

Claude does not stage, commit, push, create branches, or alter Git history.

After generation, trusted workflow steps validate the generated paths, run the
Rust token guard when required, verify that the pull request head did not change,
create one commit, and push it to the existing pull request branch.

If the pull request head changes while generation is running, publication fails
closed rather than overwriting newer work.

The privileged job does not execute project validation. After publication, the
normal GitHub CI workflows validate the resulting revision.

### Codex

Codex remains the independent pull request reviewer under the rules in
`AGENTS.md`. Generated tests, documentation, and benchmarks are reviewed as
first-class pull request changes against the same shared AI quality standard.

AI Quality uses Codex as a writer only for the existing explicit quota fallback:

```text
FROGBYTE_QUALITY_FALLBACK
```

The fallback is triggered only when the workflow confidently identifies Claude
included-usage quota exhaustion. Normal Claude failures and the configured
60-turn limit do not trigger it.

Fallback writes remain limited to the task-specific scopes in `AGENTS.md`.
Documentation fallback cannot edit Rust source because the asynchronous Codex
path does not pass through the trusted local Rust token guard.

Human maintainers remain responsible for approval and merge decisions.

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
