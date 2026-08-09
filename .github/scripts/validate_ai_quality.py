#!/usr/bin/env python3
"""Trusted scope validator for AI Quality generated changes."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass


CRATE = r"[A-Za-z0-9_-]+"
RUST_SOURCE = re.compile(rf"^crates/({CRATE})/src/.+\.rs$")
CRATE_README = re.compile(rf"^crates/({CRATE})/README\.md$")
DOCS_API = re.compile(r"^docs/api/.+")
TEST_PATH = re.compile(r"^crates/[^/]+/tests/")
BENCH_PATH = re.compile(r"^crates/[^/]+/benches/")
CRATE_ROOT = re.compile(rf"^crates/({CRATE})/src/(?:lib|main)\.rs$")

MAX_RUST_SOURCE_BYTES = 1_048_576
MAX_RUST_FILES = 20
RUSTC_TIMEOUT_SECONDS = 10

BUILTIN_DERIVES = {
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Eq",
    "Hash",
    "Ord",
    "PartialEq",
    "PartialOrd",
}

# These built-in attributes do not consume an item's token stream as an
# attribute procedural macro. cfg_attr is intentionally absent because it can
# introduce an arbitrary attribute depending on configuration.
SAFE_ATTRIBUTE_NAMES = {
    "allow",
    "cfg",
    "cold",
    "deny",
    "deprecated",
    "expect",
    "forbid",
    "ignore",
    "inline",
    "must_use",
    "no_main",
    "no_std",
    "non_exhaustive",
    "path",
    "recursion_limit",
    "repr",
    "should_panic",
    "test",
    "track_caller",
    "type_length_limit",
    "warn",
}


class ValidationError(RuntimeError):
    pass


@dataclass(frozen=True)
class InsertedLine:
    index: int
    text: str
    kind: str


def split_line_ending(line: str) -> tuple[str, str]:
    if line.endswith("\r\n"):
        return line[:-2], "\r\n"
    if line.endswith("\n"):
        return line[:-1], "\n"
    return line, ""


def classify_inserted_line(line: str) -> str | None:
    body, _ = split_line_ending(line)
    stripped = body.lstrip()

    if not stripped.startswith("//"):
        return None
    if stripped.startswith("//!"):
        return "inner-doc"
    if stripped.startswith("///") and not stripped.startswith("////"):
        return "outer-doc"
    return "comment"


def inserted_comment_lines(
    before: str,
    after: str,
    path: str,
) -> list[InsertedLine]:
    """Require Rust source changes to be insertions of whole comment lines.

    Every pre-existing line must remain byte-for-byte identical and in the same
    order. This preserves existing SAFETY annotations, block comments, code,
    attributes, strings, and Rustdoc without having to reimplement Rust lexing.
    """

    before_lines = before.splitlines(keepends=True)
    after_lines = after.splitlines(keepends=True)
    insertions: list[InsertedLine] = []

    before_index = 0
    after_index = 0

    while before_index < len(before_lines):
        if after_index >= len(after_lines):
            raise ValidationError(
                f"{path}: existing Rust lines may not be deleted or modified"
            )

        if before_lines[before_index] == after_lines[after_index]:
            before_index += 1
            after_index += 1
            continue

        kind = classify_inserted_line(after_lines[after_index])
        if kind is None:
            raise ValidationError(
                f"{path}: Rust changes may only insert whole //, ///, or //! lines"
            )

        insertions.append(
            InsertedLine(after_index, after_lines[after_index], kind)
        )
        after_index += 1

    while after_index < len(after_lines):
        kind = classify_inserted_line(after_lines[after_index])
        if kind is None:
            raise ValidationError(
                f"{path}: Rust changes may only insert whole //, ///, or //! lines"
            )
        insertions.append(
            InsertedLine(after_index, after_lines[after_index], kind)
        )
        after_index += 1

    return insertions


def blank_inserted_lines(source: str, insertions: list[InsertedLine]) -> str:
    lines = source.splitlines(keepends=True)

    for insertion in insertions:
        body, ending = split_line_ending(lines[insertion.index])
        lines[insertion.index] = (" " * len(body)) + ending

    return "".join(lines)


def rustc_environment() -> dict[str, str]:
    env = os.environ.copy()
    for name in (
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_BOOTSTRAP",
        "RUSTC_WRAPPER",
        "RUSTDOCFLAGS",
        "RUSTFLAGS",
    ):
        env.pop(name, None)

    env["LANG"] = "C"
    env["LC_ALL"] = "C"
    env["RUST_BACKTRACE"] = "0"
    return env


def rustc_unpretty(rustc: str, source: str, mode: str, path: str) -> str:
    if mode not in {"normal", "ast-tree"}:
        raise ValidationError(f"unsupported rustc unpretty mode: {mode}")

    try:
        with tempfile.TemporaryDirectory(prefix="frogbyte-ai-rust-") as tmp:
            source_path = pathlib.Path(tmp) / "input.rs"
            source_path.write_bytes(source.encode("utf-8"))

            command = [
                rustc,
                "--crate-name",
                "frogbyte_ai_quality_probe",
                "--crate-type=lib",
                "--edition=2024",
                "--color=never",
                f"-Zunpretty={mode}",
                str(source_path),
            ]

            result = subprocess.run(
                command,
                cwd=tmp,
                env=rustc_environment(),
                capture_output=True,
                text=True,
                timeout=RUSTC_TIMEOUT_SECONDS,
                check=False,
            )
    except subprocess.TimeoutExpired as error:
        raise ValidationError(
            f"{path}: trusted rustc parser timed out after "
            f"{RUSTC_TIMEOUT_SECONDS} seconds"
        ) from error

    if result.returncode != 0:
        diagnostic = result.stderr.strip()
        if len(diagnostic) > 2_000:
            diagnostic = diagnostic[-2_000:]
        raise ValidationError(
            f"{path}: trusted rustc parser rejected the source: {diagnostic}"
        )

    return result.stdout


def safe_attribute_line(line: str) -> bool:
    stripped = line.strip()
    if not (stripped.startswith("#[") or stripped.startswith("#![")):
        return True
    if not stripped.endswith("]"):
        return False

    prefix = "#![" if stripped.startswith("#![") else "#["
    body = stripped[len(prefix) : -1].strip()

    derive = re.fullmatch(r"derive\s*\(([^()]*)\)", body)
    if derive is not None:
        names = [name.strip() for name in derive.group(1).split(",")]
        return bool(names) and all(
            re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name)
            and name in BUILTIN_DERIVES
            for name in names
        )

    name = re.match(r"([A-Za-z_][A-Za-z0-9_]*)", body)
    if name is None:
        return False
    return name.group(1) in SAFE_ATTRIBUTE_NAMES


def validate_macro_sensitive_rustdoc(
    after: str,
    insertions: list[InsertedLine],
    rustc: str,
    path: str,
) -> None:
    doc_insertions = [
        insertion
        for insertion in insertions
        if insertion.kind in {"outer-doc", "inner-doc"}
    ]
    if not doc_insertions:
        return

    if any(
        insertion.kind == "inner-doc" for insertion in doc_insertions
    ) and CRATE_ROOT.fullmatch(path) is None:
        raise ValidationError(
            f"{path}: //! insertion is limited to crate root lib.rs or main.rs"
        )

    for line in after.splitlines():
        if not safe_attribute_line(line):
            raise ValidationError(
                f"{path}: Rustdoc insertion is blocked in files containing "
                "custom, active, multiline, or unknown attributes"
            )

    ast_tree = rustc_unpretty(rustc, after, "ast-tree", path)
    if re.search(r"\b(?:MacCall|MacroDef)\b", ast_tree):
        raise ValidationError(
            f"{path}: Rustdoc insertion is blocked in macro-sensitive files"
        )


def validate_insert_only_rust(
    before: str,
    after: str,
    path: str,
    rustc: str,
) -> None:
    if len(before.encode("utf-8")) > MAX_RUST_SOURCE_BYTES:
        raise ValidationError(f"{path}: Rust source is too large for AI docs")
    if len(after.encode("utf-8")) > MAX_RUST_SOURCE_BYTES:
        raise ValidationError(f"{path}: Rust source is too large for AI docs")

    insertions = inserted_comment_lines(before, after, path)
    if not insertions:
        return

    validate_macro_sensitive_rustdoc(after, insertions, rustc, path)

    # Parse both versions with the compiler's own parser. Inserted comments are
    # neutralized in a copy of the generated source. If an apparent comment line
    # was actually inserted inside a raw string, string literal, block comment,
    # or other token, the canonical parser output will still differ.
    sanitized_after = blank_inserted_lines(after, insertions)
    before_pretty = rustc_unpretty(rustc, before, "normal", path)
    after_pretty = rustc_unpretty(rustc, sanitized_after, "normal", path)

    if before_pretty != after_pretty:
        raise ValidationError(
            f"{path}: trusted Rust parsing found a non-comment syntax change"
        )


def safe_path(path: str) -> pathlib.PurePosixPath:
    pure = pathlib.PurePosixPath(path)
    if (
        not path
        or path.startswith("/")
        or "\n" in path
        or "\r" in path
        or pure.is_absolute()
        or ".." in pure.parts
    ):
        raise ValidationError(f"unsafe generated path: {path!r}")
    return pure


def git(git_dir: str, worktree: str, *args: str) -> bytes:
    return subprocess.check_output(
        [
            "git",
            f"--git-dir={git_dir}",
            f"--work-tree={worktree}",
            "-c",
            "status.renames=false",
            *args,
        ],
        stderr=subprocess.STDOUT,
    )


def changed_paths(git_dir: str, worktree: str) -> list[str]:
    status = git(
        git_dir,
        worktree,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    )
    paths: list[str] = []

    for record in status.split(b"\0"):
        if not record:
            continue
        if len(record) < 4 or record[2:3] != b" ":
            raise ValidationError("unexpected git status record")

        path = record[3:].decode("utf-8", errors="strict")
        safe_path(path)
        paths.append(path)

    return sorted(set(paths))


def parse_pr_files(raw: str) -> set[str]:
    try:
        files = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ValidationError("invalid trusted PR file snapshot JSON") from error

    if not isinstance(files, list) or not all(
        isinstance(path, str) for path in files
    ):
        raise ValidationError("trusted PR file snapshot must be a string array")

    for path in files:
        safe_path(path)
    return set(files)


def touched_crates(pr_files: set[str]) -> set[str]:
    crates: set[str] = set()
    for path in pr_files:
        parts = pathlib.PurePosixPath(path).parts
        if (
            len(parts) >= 3
            and parts[0] == "crates"
            and re.fullmatch(CRATE, parts[1])
        ):
            crates.add(parts[1])
    return crates


def validate_docs(
    paths: list[str],
    git_dir: str,
    worktree: str,
    pr_files_json: str,
    rustc: str | None,
) -> None:
    pr_files = parse_pr_files(pr_files_json)
    relevant_crates = touched_crates(pr_files)
    root = pathlib.Path(worktree)
    rust_paths = [path for path in paths if RUST_SOURCE.fullmatch(path)]

    if len(rust_paths) > MAX_RUST_FILES:
        raise ValidationError(
            f"agent:docs may edit at most {MAX_RUST_FILES} Rust files per run"
        )
    if rust_paths and not rustc:
        raise ValidationError("trusted rustc parser path is required for Rust docs")

    for path in paths:
        target = root / path
        if DOCS_API.fullmatch(path):
            continue

        readme = CRATE_README.fullmatch(path)
        if readme is not None:
            if readme.group(1) not in relevant_crates:
                raise ValidationError(
                    f"{path}: crate README is unrelated to the pull request"
                )
            continue

        if RUST_SOURCE.fullmatch(path) is None:
            raise ValidationError(
                f"agent:docs generated an out-of-scope file: {path}"
            )
        if path not in pr_files:
            raise ValidationError(
                f"{path}: Rustdoc may only touch Rust files already changed by the PR"
            )
        if not target.is_file():
            raise ValidationError(
                f"{path}: Rust source must already exist and remain a regular file"
            )

        try:
            before = git(git_dir, worktree, "show", f"HEAD:{path}").decode(
                "utf-8", errors="strict"
            )
        except subprocess.CalledProcessError as error:
            raise ValidationError(
                f"{path}: Rust source did not exist before the docs task"
            ) from error

        validate_insert_only_rust(
            before,
            target.read_text(encoding="utf-8"),
            path,
            rustc or "",
        )


def validate_changes(
    task: str,
    git_dir: str,
    worktree: str,
    pr_files_json: str,
    files_file: str,
    rustc: str | None,
) -> None:
    paths = changed_paths(git_dir, worktree)
    root = pathlib.Path(worktree)

    for path in paths:
        if (root / path).is_symlink():
            raise ValidationError(f"{path}: generated symlinks are not allowed")

    if task == "agent:tests":
        invalid = [path for path in paths if not TEST_PATH.match(path)]
    elif task == "agent:benchmarks":
        invalid = [path for path in paths if not BENCH_PATH.match(path)]
    elif task == "agent:docs":
        validate_docs(paths, git_dir, worktree, pr_files_json, rustc)
        invalid = []
    else:
        raise ValidationError(f"unknown AI quality task: {task}")

    if invalid:
        raise ValidationError(
            f"{task} generated out-of-scope files: " + ", ".join(invalid)
        )

    pathlib.Path(files_file).write_text(
        "".join(f"{path}\n" for path in paths), encoding="utf-8"
    )


def expect_rust_case(
    name: str,
    before: str,
    after: str,
    path: str,
    expected: bool,
    rustc: str,
) -> None:
    try:
        validate_insert_only_rust(before, after, path, rustc)
        valid = True
    except ValidationError:
        valid = False

    if valid != expected:
        raise ValidationError(
            f"validator self-test {name!r} expected {expected}, got {valid}"
        )


def self_test(rustc: str | None) -> None:
    if RUST_SOURCE.fullmatch("crates/example/src/lib.rs") is None:
        raise ValidationError("validator self-test rejected src/lib.rs")
    if RUST_SOURCE.fullmatch("crates/example/src/entity/mod.rs") is None:
        raise ValidationError("validator self-test rejected nested Rust source")
    if RUST_SOURCE.fullmatch("crates/example/src.rs") is not None:
        raise ValidationError("validator self-test accepted crates/*/src.rs")

    before = "pub struct A;\n"
    if len(inserted_comment_lines(before, "// Why.\n" + before, "test.rs")) != 1:
        raise ValidationError("validator self-test failed comment insertion")

    try:
        inserted_comment_lines(
            "// SAFETY[UNSAFE-001]: invariant.\nunsafe { f(); }\n",
            "unsafe { f(); }\n",
            "test.rs",
        )
    except ValidationError:
        pass
    else:
        raise ValidationError("validator self-test allowed SAFETY deletion")

    if rustc is None:
        print("AI quality validator core self-tests passed; Rust parser tests skipped.")
        return

    cases = [
        (
            "outer rustdoc",
            "pub struct A;\n",
            "/// Docs.\npub struct A;\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "ordinary explanation",
            "pub fn f() {}\n",
            "// Kept for an invariant.\npub fn f() {}\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "quote char literal",
            "const QUOTE: char = '\"';\n",
            "/// Quote.\nconst QUOTE: char = '\"';\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "quote byte literal",
            "const QUOTE: u8 = b'\"';\n",
            "/// Quote.\nconst QUOTE: u8 = b'\"';\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "builtin derive",
            "#[derive(Copy, Clone, Debug, PartialEq, Eq)]\npub struct A;\n",
            "/// Docs.\n#[derive(Copy, Clone, Debug, PartialEq, Eq)]\npub struct A;\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "custom derive",
            "#[derive(Custom)]\npub struct A;\n",
            "/// Docs.\n#[derive(Custom)]\npub struct A;\n",
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "macro input rustdoc",
            "macro_rules! emit { ($($tt:tt)*) => {}; }\nemit! {\npub struct A;\n}\n",
            "macro_rules! emit { ($($tt:tt)*) => {}; }\nemit! {\n/// Docs.\npub struct A;\n}\n",
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "raw string disguised comment",
            'const S: &str = r#"\nvalue\n"#;\n',
            'const S: &str = r#"\n// not a comment\nvalue\n"#;\n',
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "code edit",
            "pub fn value() -> u32 { 1 }\n",
            "pub fn value() -> u32 { 2 }\n",
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "existing comment edit",
            "// Existing.\npub struct A;\n",
            "// Rewritten.\npub struct A;\n",
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "block comment move",
            "/** Existing. */\npub struct A;\n",
            "pub struct A;\n/** Existing. */\n",
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "doc attribute",
            "pub struct A;\n",
            '#[doc = "Docs"]\npub struct A;\n',
            "crates/example/src/lib.rs",
            False,
        ),
        (
            "crate inner rustdoc",
            "pub mod a;\n",
            "//! Crate docs.\npub mod a;\n",
            "crates/example/src/lib.rs",
            True,
        ),
        (
            "module inner rustdoc",
            "pub struct A;\n",
            "//! Module docs.\npub struct A;\n",
            "crates/example/src/entity.rs",
            False,
        ),
    ]

    for name, before_text, after_text, path, expected in cases:
        expect_rust_case(
            name,
            before_text,
            after_text,
            path,
            expected,
            rustc,
        )

    print("AI quality validator self-tests passed.")


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--task")
    parser.add_argument("--git-dir")
    parser.add_argument("--worktree")
    parser.add_argument("--pr-files-json")
    parser.add_argument("--files-file")
    parser.add_argument("--rustc")
    return parser.parse_args()


def main() -> int:
    args = arguments()
    try:
        if args.self_test:
            self_test(args.rustc)
            return 0

        required = {
            "--task": args.task,
            "--git-dir": args.git_dir,
            "--worktree": args.worktree,
            "--pr-files-json": args.pr_files_json,
            "--files-file": args.files_file,
        }
        missing = [name for name, value in required.items() if value is None]
        if missing:
            raise ValidationError(
                "missing required arguments: " + ", ".join(missing)
            )

        validate_changes(
            args.task,
            args.git_dir,
            args.worktree,
            args.pr_files_json,
            args.files_file,
            args.rustc,
        )
        print("Generated changes satisfy the trusted AI quality scope.")
        return 0
    except (OSError, UnicodeError, ValidationError) as error:
        print(f"::error::{error}")
        return 1
    except subprocess.CalledProcessError as error:
        output = error.output.decode("utf-8", errors="replace") if error.output else ""
        print(f"::error::Git inspection failed: {output.strip()}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
