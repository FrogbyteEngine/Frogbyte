use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, strip_shebang, tokenize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodeToken {
    kind: TokenKind,
    text: String,
    separated_from_previous: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceSignature {
    shebang: Option<String>,
    code_tokens: Vec<CodeToken>,
}

enum Command {
    SelfTest,
    Verify { before: PathBuf, after: PathBuf },
}

fn is_comment(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    )
}

fn ensure_comment_is_terminated(kind: TokenKind, label: &str) -> Result<(), String> {
    if matches!(
        kind,
        TokenKind::BlockComment {
            terminated: false,
            ..
        }
    ) {
        return Err(format!("{label}: unterminated block comment"));
    }

    Ok(())
}

fn source_signature(source: &str, label: &str) -> Result<SourceSignature, String> {
    let shebang_len = strip_shebang(source).unwrap_or(0);
    let shebang = (shebang_len != 0).then(|| source[..shebang_len].to_owned());
    let body = &source[shebang_len..];

    let mut code_tokens = Vec::new();
    let mut offset = 0usize;
    let mut saw_trivia = false;

    for token in tokenize(body, FrontmatterAllowed::Yes) {
        let len = usize::try_from(token.len)
            .map_err(|_| format!("{label}: token length does not fit usize"))?;
        let end = offset
            .checked_add(len)
            .ok_or_else(|| format!("{label}: token offset overflow"))?;
        let text = body
            .get(offset..end)
            .ok_or_else(|| format!("{label}: lexer returned an invalid token range"))?;

        ensure_comment_is_terminated(token.kind, label)?;

        match token.kind {
            TokenKind::Whitespace => saw_trivia = true,
            kind if is_comment(kind) => saw_trivia = true,
            kind => {
                code_tokens.push(CodeToken {
                    kind,
                    text: text.to_owned(),
                    separated_from_previous: !code_tokens.is_empty() && saw_trivia,
                });
                saw_trivia = false;
            }
        }

        offset = end;
    }

    if offset != body.len() {
        return Err(format!(
            "{label}: lexer consumed {offset} of {} bytes",
            body.len()
        ));
    }

    Ok(SourceSignature {
        shebang,
        code_tokens,
    })
}

fn verify_signatures(before: &SourceSignature, after: &SourceSignature) -> Result<(), String> {
    if before.shebang != after.shebang {
        return Err("shebang changed".to_owned());
    }

    if before.code_tokens.len() != after.code_tokens.len() {
        return Err(format!(
            "non-comment Rust token count changed from {} to {}",
            before.code_tokens.len(),
            after.code_tokens.len()
        ));
    }

    for (index, (before_token, after_token)) in before
        .code_tokens
        .iter()
        .zip(&after.code_tokens)
        .enumerate()
    {
        if before_token.kind != after_token.kind || before_token.text != after_token.text {
            return Err(format!(
                "non-comment Rust token changed at token {index}: before {:?} {:?}, after {:?} {:?}",
                before_token.kind, before_token.text, after_token.kind, after_token.text
            ));
        }

        if before_token.separated_from_previous != after_token.separated_from_previous {
            return Err(format!(
                "lexical separation changed before non-comment token {index} ({:?})",
                before_token.text
            ));
        }
    }

    Ok(())
}

fn verify_sources(before: &str, after: &str) -> Result<(), String> {
    verify_signatures(
        &source_signature(before, "before")?,
        &source_signature(after, "after")?,
    )
}

fn verify_files(before: &Path, after: &Path) -> Result<(), String> {
    let before = fs::read_to_string(before).map_err(|error| format!("before: {error}"))?;
    let after = fs::read_to_string(after).map_err(|error| format!("after: {error}"))?;
    verify_sources(&before, &after)
}

fn expect_case(name: &str, before: &str, after: &str, expected: bool) {
    assert_eq!(
        verify_sources(before, after).is_ok(),
        expected,
        "self-test failed: {name}"
    );
}

fn self_test() {
    expect_case(
        "ordinary comment maintenance",
        "// Old.\npub struct A;\n",
        "// Better.\npub struct A;\n",
        true,
    );
    expect_case(
        "rustdoc maintenance",
        "/// Old.\npub struct A;\n",
        "/// Better.\npub struct A;\n",
        true,
    );
    expect_case(
        "SAFETY maintenance",
        "// SAFETY[UNSAFE-001]: old.\nunsafe { f(); }\n",
        "// SAFETY[UNSAFE-001]: corrected.\nunsafe { f(); }\n",
        true,
    );
    expect_case(
        "SAFETY addition",
        "unsafe { f(); }\n",
        "// SAFETY[UNSAFE-001]: invariant.\nunsafe { f(); }\n",
        true,
    );
    expect_case(
        "SAFETY deletion",
        "// SAFETY[UNSAFE-001]: obsolete.\nunsafe { f(); }\n",
        "unsafe { f(); }\n",
        true,
    );
    expect_case(
        "code edit",
        "pub fn value() -> u32 { 1 }\n",
        "pub fn value() -> u32 { 2 }\n",
        false,
    );
    expect_case(
        "string literal edit",
        "const S: &str = \"before\";\n",
        "const S: &str = \"after\";\n",
        false,
    );
    expect_case(
        "explicit doc attribute edit",
        "#[doc = \"Old.\"]\npub struct A;\n",
        "#[doc = \"New.\"]\npub struct A;\n",
        false,
    );
    expect_case(
        "raw string content edit",
        "const S: &str = r#\"value\"#;\n",
        "const S: &str = r#\"// comment\"#;\n",
        false,
    );
    expect_case(
        "punctuation jointness",
        "macro_rules! m { () => { call!(+ /* gap */ =); }; }\n",
        "macro_rules! m { () => { call!(+=); }; }\n",
        false,
    );

    println!("AI quality token guard self-tests passed.");
}

fn parse_command() -> Result<Command, String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args == ["--self-test"] {
        return Ok(Command::SelfTest);
    }

    let mut before = None;
    let mut after = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--before" => {
                index += 1;
                before = args.get(index).map(PathBuf::from);
            }
            "--after" => {
                index += 1;
                after = args.get(index).map(PathBuf::from);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }

    Ok(Command::Verify {
        before: before.ok_or_else(|| "missing --before".to_owned())?,
        after: after.ok_or_else(|| "missing --after".to_owned())?,
    })
}

fn main() -> ExitCode {
    let result = match parse_command() {
        Ok(Command::SelfTest) => {
            self_test();
            Ok(())
        }
        Ok(Command::Verify { before, after }) => verify_files(&before, &after),
        Err(error) => Err(error),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::verify_sources;

    #[test]
    fn accepts_comment_and_safety_maintenance() {
        assert!(verify_sources("// Old.\npub struct A;\n", "// New.\npub struct A;\n").is_ok());
        assert!(
            verify_sources(
                "// SAFETY[UNSAFE-001]: old.\nunsafe { f(); }\n",
                "// SAFETY[UNSAFE-001]: corrected.\nunsafe { f(); }\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_comment_token_changes() {
        assert!(
            verify_sources(
                "pub fn value() -> u32 { 1 }\n",
                "pub fn value() -> u32 { 2 }\n"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_punctuation_jointness_changes() {
        assert!(
            verify_sources(
                "macro_rules! m { () => { call!(+ /* gap */ =); }; }\n",
                "macro_rules! m { () => { call!(+=); }; }\n"
            )
            .is_err()
        );
    }
}
