# Continuous Integration

## Purpose

Frogbyte uses GitHub Actions to validate pull requests targeting `main` and pushes to `main`.

The required CI checks cover formatting, linting, tests, documentation, and the Windows release build.

## Required local validation

Before marking a pull request ready for review, run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked --no-fail-fast
cargo test --workspace --doc --all-features --locked --no-fail-fast
cargo build --workspace --all-targets --all-features --release --locked
```

Validate Rust documentation with:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --all-features --no-deps --locked
```

## Toolchain and dependencies

The Rust toolchain is defined by `rust-toolchain.toml`.

`Cargo.lock` is committed and CI uses `--locked` to keep dependency resolution reproducible.

## Specialized validation

Miri runs separately for compatible ECS changes.

Security checks and scheduled compatibility checks are maintained in dedicated workflows and do not replace the required CI checks.

## Merge requirement

Required CI checks must pass before merging a pull request.
