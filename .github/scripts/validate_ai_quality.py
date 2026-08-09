#!/usr/bin/env python3
"""Trusted scope validator for AI Quality generated changes."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys


CRATE = r"[A-Za-z0-9_-]+"
RUST_SOURCE = re.compile(rf"^crates/({CRATE})/src(?:/.+)?\.rs$")
CRATE_README = re.compile(rf"^crates/({CRATE})/README\.md$")
DOCS_API = re.compile(r"^docs/api/.+")
TEST_PATH = re.compile(r"^crates/[^/]+/tests/")
BENCH_PATH = re.compile(r"^crates/[^/]+/benches/")


class ValidationError(RuntimeError):
    pass


def raw_string_end(source: str, start: int) -> int | None:
    for prefix in ("br", "cr", "r"):
        if not source.startswith(prefix, start):
            continue

        cursor = start + len(prefix)
        hashes = 0
        while cursor < len(source) and source[cursor] == "#":
            hashes += 1
            cursor += 1

        if cursor >= len(source) or source[cursor] != '"':
            continue

        marker = '"' + ("#" * hashes)
        end = source.find(marker, cursor + 1)
        if end < 0:
            raise ValidationError(
                f"unterminated raw string at character offset {start}"
            )
        return end + len(marker)

    return None


def string_end(source: str, start: int) -> int | None:
    for prefix in ("b", "c", ""):
        opener = prefix + '"'
        if not source.startswith(opener, start):
            continue

        cursor = start + len(opener)
        escaped = False
        while cursor < len(source):
            char = source[cursor]
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                return cursor + 1
            cursor += 1

        raise ValidationError(
            f"unterminated string literal at character offset {start}"
        )

    return None


def block_comment_end(source: str, start: int) -> int:
    depth = 1
    cursor = start + 2
    while cursor < len(source):
        if source.startswith("/*", cursor):
            depth += 1
            cursor += 2
            continue
        if source.startswith("*/", cursor):
            depth -= 1
            cursor += 2
            if depth == 0:
                return cursor
            continue
        cursor += 1

    raise ValidationError(
        f"unterminated block comment at character offset {start}"
    )


def scan_rust(source: str) -> tuple[str, tuple[str, ...]]:
    """Return normalized non-comment text and exact block comments.

    Rust string, byte-string, raw-string, and C-string literals stay opaque, so
    comment markers inside literals cannot be mistaken for actual comments.
    """

    program: list[str] = []
    blocks: list[str] = []
    pending_space = False
    cursor = 0

    def flush_space() -> None:
        nonlocal pending_space
        if pending_space and program and program[-1] != " ":
            program.append(" ")
        pending_space = False

    while cursor < len(source):
        end = raw_string_end(source, cursor)
        if end is not None:
            flush_space()
            program.append(source[cursor:end])
            cursor = end
            continue

        end = string_end(source, cursor)
        if end is not None:
            flush_space()
            program.append(source[cursor:end])
            cursor = end
            continue

        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            cursor = len(source) if end < 0 else end
            pending_space = True
            continue

        if source.startswith("/*", cursor):
            end = block_comment_end(source, cursor)
            blocks.append(source[cursor:end])
            cursor = end
            pending_space = True
            continue

        char = source[cursor]
        if char.isspace():
            pending_space = True
        else:
            flush_space()
            program.append(char)
        cursor += 1

    return "".join(program).strip(), tuple(blocks)


def validate_comment_only_rust(before: str, after: str, path: str) -> None:
    before_program, before_blocks = scan_rust(before)
    after_program, after_blocks = scan_rust(after)

    if before_program != after_program:
        raise ValidationError(
            f"{path}: Rust program text changed; only line comments are allowed"
        )
    if before_blocks != after_blocks:
        raise ValidationError(
            f"{path}: block comments changed; use only //, ///, or //! comments"
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
) -> None:
    pr_files = parse_pr_files(pr_files_json)
    relevant_crates = touched_crates(pr_files)
    root = pathlib.Path(worktree)

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

        validate_comment_only_rust(
            before,
            target.read_text(encoding="utf-8"),
            path,
        )


def validate_changes(
    task: str,
    git_dir: str,
    worktree: str,
    pr_files_json: str,
    files_file: str,
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
        validate_docs(paths, git_dir, worktree, pr_files_json)
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


def self_test() -> None:
    cases = [
        ("pub struct A;\n", "/// Docs.\npub struct A;\n", True),
        ("pub mod a;\n", "//! Module docs.\npub mod a;\n", True),
        ("let x = 1;\n", "// Why.\nlet x = 1;\n", True),
        ("let x = 1;\n", "let x = 2;\n", False),
        ("pub struct A;\n", '#[doc = "Docs"]\npub struct A;\n', False),
        ("pub struct A;\n", "/** Docs. */\npub struct A;\n", False),
        ("pub struct A;\n", "/* Docs. */\npub struct A;\n", False),
        (
            'const S: &str = r#"// text /* text */"#;\n',
            '/// Docs.\nconst S: &str = r#"// text /* text */"#;\n',
            True,
        ),
        (
            'const S: &str = r#"// one"#;\n',
            'const S: &str = r#"// two"#;\n',
            False,
        ),
        (
            'const U: &str = "https://example.invalid/a/*b*/";\n',
            '/// Docs.\nconst U: &str = "https://example.invalid/a/*b*/";\n',
            True,
        ),
        (
            "fn id<'a>(x: &'a str) -> &'a str { x }\n",
            "/// Docs.\nfn id<'a>(x: &'a str) -> &'a str { x }\n",
            True,
        ),
        (
            "/* a /* nested */ b */\nfn f() {}\n",
            "/* a /* nested */ b */\n/// Docs.\nfn f() {}\n",
            True,
        ),
        (
            'const B: &[u8] = br#"//"#;\n',
            '/// Docs.\nconst B: &[u8] = br#"//"#;\n',
            True,
        ),
        (
            'const C: &core::ffi::CStr = cr#"/* */"#;\n',
            '/// Docs.\nconst C: &core::ffi::CStr = cr#"/* */"#;\n',
            True,
        ),
    ]

    for before, after, expected in cases:
        try:
            validate_comment_only_rust(before, after, "test.rs")
            valid = True
        except ValidationError:
            valid = False
        if valid != expected:
            raise ValidationError(
                f"validator self-test expected {expected}, got {valid}"
            )


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--task")
    parser.add_argument("--git-dir")
    parser.add_argument("--worktree")
    parser.add_argument("--pr-files-json")
    parser.add_argument("--files-file")
    return parser.parse_args()


def main() -> int:
    args = arguments()
    try:
        if args.self_test:
            self_test()
            print("AI quality validator self-test passed.")
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
