use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use std::env;
use std::fs;
use std::panic;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DelimiterKind {
    Parenthesis,
    Brace,
    Bracket,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpacingKind {
    Alone,
    Joint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Node {
    Group(DelimiterKind, Vec<Node>),
    Ident(String),
    Punct(char, SpacingKind),
    Literal(String),
    DocAttr { inner: bool, literal: String },
}

enum Command {
    SelfTest,
    Verify {
        before: PathBuf,
        after: PathBuf,
        allowed_doc_comments: usize,
    },
}

fn delimiter_kind(delimiter: Delimiter) -> DelimiterKind {
    match delimiter {
        Delimiter::Parenthesis => DelimiterKind::Parenthesis,
        Delimiter::Brace => DelimiterKind::Brace,
        Delimiter::Bracket => DelimiterKind::Bracket,
        Delimiter::None => DelimiterKind::None,
    }
}

fn spacing_kind(spacing: Spacing) -> SpacingKind {
    match spacing {
        Spacing::Alone => SpacingKind::Alone,
        Spacing::Joint => SpacingKind::Joint,
    }
}

fn punct_is(tree: &TokenTree, expected: char) -> bool {
    matches!(tree, TokenTree::Punct(punct) if punct.as_char() == expected)
}

fn doc_attribute_literal(group: &proc_macro2::Group) -> Option<String> {
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }

    let body: Vec<_> = group.stream().into_iter().collect();
    if body.len() != 3 {
        return None;
    }

    match (&body[0], &body[1], &body[2]) {
        (TokenTree::Ident(ident), TokenTree::Punct(equals), TokenTree::Literal(literal))
            if ident == "doc" && equals.as_char() == '=' =>
        {
            Some(literal.to_string())
        }
        _ => None,
    }
}

fn doc_attribute_at(trees: &[TokenTree], index: usize) -> Option<(Node, usize)> {
    if !punct_is(trees.get(index)?, '#') {
        return None;
    }

    if let Some(TokenTree::Group(group)) = trees.get(index + 1)
        && let Some(literal) = doc_attribute_literal(group)
    {
        return Some((
            Node::DocAttr {
                inner: false,
                literal,
            },
            2,
        ));
    }

    if punct_is(trees.get(index + 1)?, '!')
        && let Some(TokenTree::Group(group)) = trees.get(index + 2)
        && let Some(literal) = doc_attribute_literal(group)
    {
        return Some((
            Node::DocAttr {
                inner: true,
                literal,
            },
            3,
        ));
    }

    None
}

fn normalize_stream(stream: TokenStream) -> Vec<Node> {
    let trees: Vec<_> = stream.into_iter().collect();
    let mut nodes = Vec::with_capacity(trees.len());
    let mut index = 0;

    while index < trees.len() {
        if let Some((doc, consumed)) = doc_attribute_at(&trees, index) {
            nodes.push(doc);
            index += consumed;
            continue;
        }

        nodes.push(normalize_tree(trees[index].clone()));
        index += 1;
    }

    nodes
}

fn normalize_tree(tree: TokenTree) -> Node {
    match tree {
        TokenTree::Group(group) => Node::Group(
            delimiter_kind(group.delimiter()),
            normalize_stream(group.stream()),
        ),
        TokenTree::Ident(ident) => Node::Ident(ident.to_string()),
        TokenTree::Punct(punct) => Node::Punct(punct.as_char(), spacing_kind(punct.spacing())),
        TokenTree::Literal(literal) => Node::Literal(literal.to_string()),
    }
}

fn parse_source(source: &str, label: &str) -> Result<Vec<Node>, String> {
    let parsed = panic::catch_unwind(|| TokenStream::from_str(source))
        .map_err(|_| format!("{label}: proc-macro2 panicked while tokenizing source"))?
        .map_err(|error| format!("{label}: could not tokenize Rust source: {error}"))?;

    Ok(normalize_stream(parsed))
}

fn compare_node(
    before: &Node,
    after: &Node,
    remaining_doc_insertions: &mut usize,
) -> Result<(), String> {
    match (before, after) {
        (
            Node::Group(before_delimiter, before_nodes),
            Node::Group(after_delimiter, after_nodes),
        ) if before_delimiter == after_delimiter => {
            compare_stream(before_nodes, after_nodes, remaining_doc_insertions)
        }
        _ if before == after => Ok(()),
        _ => Err("non-documentation Rust token changed".to_owned()),
    }
}

fn compare_stream(
    before: &[Node],
    after: &[Node],
    remaining_doc_insertions: &mut usize,
) -> Result<(), String> {
    let mut before_index = 0;
    let mut after_index = 0;

    while before_index < before.len() {
        if after_index >= after.len() {
            return Err("Rust tokens were removed".to_owned());
        }

        let checkpoint = *remaining_doc_insertions;
        if compare_node(
            &before[before_index],
            &after[after_index],
            remaining_doc_insertions,
        )
        .is_ok()
        {
            before_index += 1;
            after_index += 1;
            continue;
        }
        *remaining_doc_insertions = checkpoint;

        if matches!(after[after_index], Node::DocAttr { .. }) && *remaining_doc_insertions > 0 {
            *remaining_doc_insertions -= 1;
            after_index += 1;
            continue;
        }

        return Err(format!(
            "unexpected token difference near before token {before_index} and after token {after_index}"
        ));
    }

    while after_index < after.len() {
        if matches!(after[after_index], Node::DocAttr { .. }) && *remaining_doc_insertions > 0 {
            *remaining_doc_insertions -= 1;
            after_index += 1;
            continue;
        }

        return Err(format!(
            "unexpected Rust token inserted at after token {after_index}"
        ));
    }

    Ok(())
}

fn verify_sources(before: &str, after: &str, allowed_doc_comments: usize) -> Result<(), String> {
    let before_nodes = parse_source(before, "before")?;
    let after_nodes = parse_source(after, "after")?;
    let mut remaining = allowed_doc_comments;

    compare_stream(&before_nodes, &after_nodes, &mut remaining)?;

    if remaining != 0 {
        let observed = allowed_doc_comments - remaining;
        return Err(format!(
            "expected {allowed_doc_comments} inserted Rustdoc comments but tokenization observed only {observed}"
        ));
    }

    Ok(())
}

fn read_utf8(path: &Path, label: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("{label}: {error}"))?;
    String::from_utf8(bytes).map_err(|error| format!("{label}: source is not UTF-8: {error}"))
}

fn verify_files(before: &Path, after: &Path, allowed_doc_comments: usize) -> Result<(), String> {
    let before_source = read_utf8(before, "before")?;
    let after_source = read_utf8(after, "after")?;
    verify_sources(&before_source, &after_source, allowed_doc_comments)
}

fn expect_case(name: &str, before: &str, after: &str, allowed: usize, expected: bool) {
    let valid = verify_sources(before, after, allowed).is_ok();
    assert_eq!(valid, expected, "self-test failed: {name}");
}

fn self_test() {
    expect_case(
        "ordinary comment",
        "pub struct A;\n",
        "// Explanation.\npub struct A;\n",
        0,
        true,
    );
    expect_case(
        "outer rustdoc",
        "pub struct A;\n",
        "/// Documentation.\npub struct A;\n",
        1,
        true,
    );
    expect_case(
        "inner rustdoc",
        "pub mod entity;\n",
        "//! Module documentation.\npub mod entity;\n",
        1,
        true,
    );
    expect_case(
        "multiple rustdoc lines",
        "pub struct A;\n",
        "/// First line.\n/// Second line.\npub struct A;\n",
        2,
        true,
    );
    expect_case(
        "macro source location shift",
        "const LINE: u32 = line!();\n",
        "// Explanation.\nconst LINE: u32 = line!();\n",
        0,
        true,
    );
    expect_case(
        "track caller source location shift",
        "fn f() { None::<u8>.unwrap(); }\n",
        "/// Documentation.\nfn f() { None::<u8>.unwrap(); }\n",
        1,
        true,
    );
    expect_case(
        "custom attribute unchanged",
        "#[custom]\npub struct A;\n",
        "/// Documentation.\n#[custom]\npub struct A;\n",
        1,
        true,
    );
    expect_case(
        "macro words in literal",
        "pub const S: &str = \"MacCall MacroDef\";\n",
        "/// Documentation.\npub const S: &str = \"MacCall MacroDef\";\n",
        1,
        true,
    );
    expect_case(
        "code edit",
        "pub fn value() -> u32 { 1 }\n",
        "pub fn value() -> u32 { 2 }\n",
        0,
        false,
    );
    expect_case(
        "string edit",
        "const S: &str = \"before\";\n",
        "const S: &str = \"after\";\n",
        0,
        false,
    );
    expect_case(
        "ordinary comment text inside raw string",
        "const S: &str = r#\"\nvalue\n\"#;\n",
        "const S: &str = r#\"\n// not a comment\nvalue\n\"#;\n",
        0,
        false,
    );
    expect_case(
        "rustdoc text inside raw string",
        "const S: &str = r#\"\nvalue\n\"#;\n",
        "const S: &str = r#\"\n/// not rustdoc\nvalue\n\"#;\n",
        1,
        false,
    );
    expect_case(
        "existing rustdoc changed",
        "/// Old documentation.\npub struct A;\n",
        "/// New documentation.\npub struct A;\n",
        0,
        false,
    );
    expect_case(
        "explicit doc attribute inserted without allowance",
        "pub struct A;\n",
        "#[doc = \"Documentation.\"]\npub struct A;\n",
        0,
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
    let mut allowed_doc_comments = None;
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
            "--allowed-doc-comments" => {
                index += 1;
                allowed_doc_comments = Some(
                    args.get(index)
                        .ok_or_else(|| "missing value for --allowed-doc-comments".to_owned())?
                        .parse::<usize>()
                        .map_err(|_| "--allowed-doc-comments must be an integer".to_owned())?,
                );
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }

    Ok(Command::Verify {
        before: before.ok_or_else(|| "missing --before".to_owned())?,
        after: after.ok_or_else(|| "missing --after".to_owned())?,
        allowed_doc_comments: allowed_doc_comments
            .ok_or_else(|| "missing --allowed-doc-comments".to_owned())?,
    })
}

fn main() -> ExitCode {
    let result = match parse_command() {
        Ok(Command::SelfTest) => {
            self_test();
            Ok(())
        }
        Ok(Command::Verify {
            before,
            after,
            allowed_doc_comments,
        }) => verify_files(&before, &after, allowed_doc_comments),
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
    fn accepts_only_added_outer_rustdoc() {
        assert!(
            verify_sources("pub struct A;\n", "/// Documentation.\npub struct A;\n", 1).is_ok()
        );
    }

    #[test]
    fn accepts_source_location_changes_when_tokens_are_unchanged() {
        assert!(
            verify_sources(
                "const LINE: u32 = line!();\n",
                "// Explanation.\nconst LINE: u32 = line!();\n",
                0
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_doc_token_changes() {
        assert!(
            verify_sources(
                "pub fn value() -> u32 { 1 }\n",
                "pub fn value() -> u32 { 2 }\n",
                0
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_fake_doc_comment_inside_raw_string() {
        assert!(
            verify_sources(
                "const S: &str = r#\"\nvalue\n\"#;\n",
                "const S: &str = r#\"\n/// not rustdoc\nvalue\n\"#;\n",
                1
            )
            .is_err()
        );
    }
}
