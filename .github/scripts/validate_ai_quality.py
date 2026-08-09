#!/usr/bin/env python3
"""Trusted scope validator for AI Quality generated changes."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import tempfile
from dataclasses import dataclass


CRATE = r"[A-Za-z0-9_-]+"
RUST_SOURCE = re.compile(rf"^crates/({CRATE})/src/.+\.rs$")
CRATE_README = re.compile(rf"^crates/({CRATE})/README\.md$")
DOCS_API = re.compile(r"^docs/api/.+")
TEST_PATH = re.compile(r"^crates/[^/]+/tests/")
BENCH_PATH = re.compile(r"^crates/[^/]+/benches/")

MAX_RUST_SOURCE_BYTES = 1_048_576
MAX_RUST_FILES = 20
TOKEN_GUARD_TIMEOUT_SECONDS = 10


class ValidationError(RuntimeError):
    pass


@dataclass(frozen=True)
class InsertedLine:
    index: int
    data: bytes
    kind: str


def split_lf_lines(data: bytes) -> list[bytes]:
    """Split only at LF while preserving exact source bytes and CRLF."""

    lines: list[bytes] = []
    start = 0

    while True:
        newline = data.find(b"\n", start)
        if newline < 0:
            break
        lines.append(data[start : newline + 1])
        start = newline + 1

    if start < len(data):
        lines.append(data[start:])

    return lines


def classify_inserted_line(line: bytes) -> str | None:
    body = line
    if body.endswith(b"\r\n"):
        body = body[:-2]
    elif body.endswith(b"\n"):
        body = body[:-1]

    stripped = body.lstrip(b" \t")

    if not stripped.startswith(b"//"):
        return None
    if stripped.startswith(b"//!"):
        return "inner-doc"
    if stripped.startswith(b"///") and not stripped.startswith(b"////"):
        return "outer-doc"
    return "comment"


def inserted_comment_lines(
    before: bytes,
    after: bytes,
    path: str,
) -> list[InsertedLine]:
    """Require Rust edits to be insertions of physical //-prefixed lines.

    Every pre-existing source line must remain byte-for-byte identical and in
    the same order. A separate trusted token guard verifies that these physical
    insertions do not alter pre-existing Rust tokens.
    """

    before_lines = split_lf_lines(before)
    after_lines = split_lf_lines(after)
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
                f"{path}: Rust changes may only insert whole //-prefixed lines"
            )

        insertions.append(
            InsertedLine(after_index, after_lines[after_index], kind)
        )
        after_index += 1

    while after_index < len(after_lines):
        kind = classify_inserted_line(after_lines[after_index])
        if kind is None:
            raise ValidationError(
                f"{path}: Rust changes may only insert whole //-prefixed lines"
            )

        insertions.append(
            InsertedLine(after_index, after_lines[after_index], kind)
        )
        after_index += 1

    return insertions


def validate_utf8(data: bytes, path: str) -> None:
    try:
        data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise ValidationError(f"{path}: Rust source is not valid UTF-8") from error


def run_token_guard(
    token_guard: str,
    before: bytes,
    after: bytes,
    allowed_doc_comments: int,
    path: str,
) -> None:
    with tempfile.TemporaryDirectory(prefix="frogbyte-ai-token-guard-") as tmp:
        root = pathlib.Path(tmp)
        before_path = root / "before.rs"
        after_path = root / "after.rs"
        before_path.write_bytes(before)
        after_path.write_bytes(after)

        try:
            result = subprocess.run(
                [
                    token_guard,
                    "--before",
                    str(before_path),
                    "--after",
                    str(after_path),
                    "--allowed-doc-comments",
                    str(allowed_doc_comments),
                ],
                capture_output=True,
                text=True,
                timeout=TOKEN_GUARD_TIMEOUT_SECONDS,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise ValidationError(
                f"{path}: trusted token guard timed out after "
                f"{TOKEN_GUARD_TIMEOUT_SECONDS} seconds"
            ) from error

    if result.returncode == 0:
        return

    diagnostic = (result.stderr or result.stdout).strip()
    if len(diagnostic) > 2_000:
        diagnostic = diagnostic[-2_000:]
    raise ValidationError(
        f"{path}: trusted Rust token-integrity check failed: {diagnostic}"
    )


def validate_insert_only_rust(
    before: bytes,
    after: bytes,
    path: str,
    token_guard: str,
) -> None:
    if len(before) > MAX_RUST_SOURCE_BYTES:
        raise ValidationError(f"{path}: Rust source is too large for AI docs")
    if len(after) > MAX_RUST_SOURCE_BYTES:
        raise ValidationError(f"{path}: Rust source is too large for AI docs")

    validate_utf8(before, path)
    validate_utf8(after, path)

    insertions = inserted_comment_lines(before, after, path)
    if not insertions:
        return

    allowed_doc_comments = sum(
        insertion.kind in {"outer-doc", "inner-doc"}
        for insertion in insertions
    )
    run_token_guard(
        token_guard,
        before,
        after,
        allowed_doc_comments,
        path,
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
    token_guard: str | None,
) -> None:
    pr_files = parse_pr_files(pr_files_json)
    relevant_crates = touched_crates(pr_files)
    root = pathlib.Path(worktree)
    rust_paths = [path for path in paths if RUST_SOURCE.fullmatch(path)]

    if len(rust_paths) > MAX_RUST_FILES:
        raise ValidationError(
            f"agent:docs may edit at most {MAX_RUST_FILES} Rust files per run"
        )
    if rust_paths and not token_guard:
        raise ValidationError(
            "trusted token guard path is required for Rust documentation"
        )

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
                f"{path}: Rust docs may only touch Rust files already changed by the PR"
            )
        if not target.is_file():
            raise ValidationError(
                f"{path}: Rust source must already exist and remain a regular file"
            )

        try:
            before = git(git_dir, worktree, "show", f"HEAD:{path}")
        except subprocess.CalledProcessError as error:
            raise ValidationError(
                f"{path}: Rust source did not exist before the docs task"
            ) from error

        after = target.read_bytes()

        validate_insert_only_rust(
            before,
            after,
            path,
            token_guard or "",
        )


def validate_changes(
    task: str,
    git_dir: str,
    worktree: str,
    pr_files_json: str,
    files_file: str,
    token_guard: str | None,
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
        validate_docs(
            paths,
            git_dir,
            worktree,
            pr_files_json,
            token_guard,
        )
        invalid = []
    else:
        raise ValidationError(f"unknown AI quality task: {task}")

    if invalid:
        raise ValidationError(
            f"{task} generated out-of-scope files: " + ", ".join(invalid)
        )

    pathlib.Path(files_file).write_text(
        "".join(f"{path}\n" for path in paths),
        encoding="utf-8",
    )


def expect_rust_case(
    name: str,
    before: bytes,
    after: bytes,
    expected: bool,
    token_guard: str,
) -> None:
    try:
        validate_insert_only_rust(
            before,
            after,
            "crates/example/src/lib.rs",
            token_guard,
        )
        valid = True
    except ValidationError:
        valid = False

    if valid != expected:
        raise ValidationError(
            f"validator self-test {name!r} expected {expected}, got {valid}"
        )


def self_test(token_guard: str | None) -> None:
    if RUST_SOURCE.fullmatch("crates/example/src/lib.rs") is None:
        raise ValidationError("validator self-test rejected src/lib.rs")
    if RUST_SOURCE.fullmatch("crates/example/src/entity/mod.rs") is None:
        raise ValidationError("validator self-test rejected nested Rust source")
    if RUST_SOURCE.fullmatch("crates/example/src.rs") is not None:
        raise ValidationError("validator self-test accepted crates/*/src.rs")

    try:
        inserted_comment_lines(
            b"// SAFETY[UNSAFE-001]: invariant.\nunsafe { f(); }\n",
            b"unsafe { f(); }\n",
            "test.rs",
        )
    except ValidationError:
        pass
    else:
        raise ValidationError("validator self-test allowed SAFETY deletion")

    try:
        inserted_comment_lines(
            b"/** Existing. */\npub struct A;\n",
            b"pub struct A;\n/** Existing. */\n",
            "test.rs",
        )
    except ValidationError:
        pass
    else:
        raise ValidationError("validator self-test allowed block comment move")

    if token_guard is None:
        print(
            "AI quality validator core self-tests passed; "
            "token-integrity tests skipped."
        )
        return

    cases = [
        (
            "ordinary comment",
            b"pub struct A;\n",
            b"// Explanation.\npub struct A;\n",
            True,
        ),
        (
            "outer rustdoc",
            b"pub struct A;\n",
            b"/// Documentation.\npub struct A;\n",
            True,
        ),
        (
            "inner module rustdoc",
            b"pub struct A;\n",
            b"//! Module documentation.\npub struct A;\n",
            True,
        ),
        (
            "quote character literal",
            b'const QUOTE: char = \'\\"\';\n',
            b'/// Quote.\nconst QUOTE: char = \'\\"\';\n',
            True,
        ),
        (
            "quote byte literal",
            b'const QUOTE: u8 = b\'\\"\';\n',
            b'/// Quote.\nconst QUOTE: u8 = b\'\\"\';\n',
            True,
        ),
        (
            "custom attribute unchanged",
            b"#[custom]\npub struct A;\n",
            b"/// Documentation.\n#[custom]\npub struct A;\n",
            True,
        ),
        (
            "attribute after another item",
            b"pub struct A; #[custom] pub struct B;\n",
            b"/// Documentation.\npub struct A; #[custom] pub struct B;\n",
            True,
        ),
        (
            "macro source-location shift",
            b"const LINE: u32 = line!();\n",
            b"// Explanation.\nconst LINE: u32 = line!();\n",
            True,
        ),
        (
            "track-caller source-location shift",
            b"fn f() { None::<u8>.unwrap(); }\n",
            b"/// Documentation.\nfn f() { None::<u8>.unwrap(); }\n",
            True,
        ),
        (
            "macro words in literal",
            b'pub const S: &str = "MacCall MacroDef";\n',
            b'/// Documentation.\npub const S: &str = "MacCall MacroDef";\n',
            True,
        ),
        (
            "CRLF source",
            b"pub struct A;\r\n",
            b"/// Documentation.\r\npub struct A;\r\n",
            True,
        ),
        (
            "code edit",
            b"pub fn value() -> u32 { 1 }\n",
            b"pub fn value() -> u32 { 2 }\n",
            False,
        ),
        (
            "existing comment edit",
            b"// Existing.\npub struct A;\n",
            b"// Rewritten.\npub struct A;\n",
            False,
        ),
        (
            "existing rustdoc edit",
            b"/// Existing.\npub struct A;\n",
            b"/// Rewritten.\npub struct A;\n",
            False,
        ),
        (
            "explicit doc attribute",
            b"pub struct A;\n",
            b'#[doc = "Documentation."]\npub struct A;\n',
            False,
        ),
        (
            "raw string fake ordinary comment",
            b'const S: &str = r#"\nvalue\n"#;\n',
            b'const S: &str = r#"\n// not a comment\nvalue\n"#;\n',
            False,
        ),
        (
            "raw string fake rustdoc",
            b'const S: &str = r#"\nvalue\n"#;\n',
            b'const S: &str = r#"\n/// not rustdoc\nvalue\n"#;\n',
            False,
        ),
        (
            "block rustdoc content insertion",
            b"/** Existing documentation. */\npub struct A;\n",
            b"/**\n// inserted physical line\nExisting documentation. */\npub struct A;\n",
            False,
        ),
    ]

    for name, before, after, expected in cases:
        expect_rust_case(
            name,
            before,
            after,
            expected,
            token_guard,
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
    parser.add_argument("--token-guard")
    return parser.parse_args()


def main() -> int:
    args = arguments()

    try:
        if args.self_test:
            self_test(args.token_guard)
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
            args.token_guard,
        )
        print("Generated changes satisfy the trusted AI quality scope.")
        return 0
    except (OSError, UnicodeError, ValidationError) as error:
        print(f"::error::{error}")
        return 1
    except subprocess.CalledProcessError as error:
        output = (
            error.output.decode("utf-8", errors="replace")
            if error.output
            else ""
        )
        print(f"::error::Git inspection failed: {output.strip()}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
