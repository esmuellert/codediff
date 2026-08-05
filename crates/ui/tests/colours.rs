//! What each language actually comes out as.
//!
//! [`scopes`](../scopes.rs) proves every selector matches *something*;
//! this proves the right one wins. They are different failures: a table where
//! `keyword` outranks `storage.type.function` matches everything and colours
//! `fn` wrongly, and only an assertion about a particular word can say so.
//!
//! Each case names a word in a snippet and the role it must end up wearing.
//! Snippets are small and self-contained so the expected answer is visible
//! next to the source.

use syntax::{Clues, Engine, Highlighted, Palette};
use ui::theme::Token;

/// The token covering the first occurrence of `needle`.
///
/// **Deliberately does not say which engine.** These assertions are about the
/// language — in Rust, `fn` is a keyword — and a reader does not care which
/// machinery reached that answer. Asking through the seam means each case is
/// checked against whatever actually runs, and that a language moving from one
/// engine to the other is caught by the answer changing rather than by nothing
/// at all.
///
/// `None` means no rule claimed it, which for these cases is a failure — the
/// point of each is that something specific claims it.
fn token_of(path: &str, source: &str, needle: &str) -> Option<Token> {
    read(path, source, needle)
}

fn read(path: &str, source: &str, needle: &str) -> Option<Token> {
    let engine = Engine::new();
    let palette = Palette::new(
        &ui::theme::scopes::rules(),
        &ui::theme::captures::captures(),
    );
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let grammar = engine
        .find(
            Clues::new(path, lines.first().map(String::as_str)),
            lines.len(),
        )
        .unwrap_or_else(|| panic!("no grammar claims {path}"));
    let mut read = Highlighted::new(&engine, grammar, &palette, &lines);
    read.reach(&engine, &palette, lines.len() as u32, &lines);

    let (n, byte) = lines
        .iter()
        .enumerate()
        .find_map(|(n, line)| line.find(needle).map(|b| (n, b as u32)))
        .unwrap_or_else(|| panic!("{needle:?} is not in the snippet"));

    read.line(n as u32)
        .iter()
        .find(|span| span.bytes.contains(&byte))
        .and_then(|span| span.style.pen)
        .and_then(ui::theme::token)
}

fn check(cases: &[(&str, &str, &str, Token)]) {
    let mut wrong = Vec::new();
    for (path, source, needle, want) in cases {
        let got = token_of(path, source, needle);
        if got != Some(*want) {
            wrong.push(format!(
                "{path}: {needle:?} is {} but should be {}",
                got.map_or("nothing", Token::name),
                want.name(),
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

const RUST: &str = r##"//! A doc comment.
use std::fmt;

#[derive(Debug)]
pub struct Widget {
    label: String,
}

impl Widget {
    pub fn render(&self, times: u32) -> String {
        let mark = '\t';
        println!("{}", self.label);
        format!("{mark}{}", times + 1)
    }
}
"##;

#[test]
fn rust() {
    check(&[
        ("a.rs", RUST, "//! A doc", Token::Comment),
        // `fn` is `storage.type.function`, not a keyword. A table with only
        // `keyword` in it misses this, which is why the table is scope paths.
        ("a.rs", RUST, "fn ", Token::Keyword),
        ("a.rs", RUST, "pub", Token::Keyword),
        ("a.rs", RUST, "let ", Token::Keyword),
        ("a.rs", RUST, "u32", Token::Keyword),
        ("a.rs", RUST, "use ", Token::Keyword),
        ("a.rs", RUST, "Widget", Token::Type),
        ("a.rs", RUST, "render", Token::Function),
        ("a.rs", RUST, "self.label", Token::Builtin),
        ("a.rs", RUST, "times", Token::Parameter),
        ("a.rs", RUST, "derive", Token::Attribute),
        ("a.rs", RUST, "'\\t'", Token::Character),
        ("a.rs", RUST, "println", Token::Library),
        ("a.rs", RUST, "1", Token::Constant),
    ]);
}

const TYPESCRIPT: &str = r#"// A comment.
import { readFile } from "node:fs";

@sealed
export class Widget implements Shape {
    private pattern = /^\d+$/g;

    render(times: number): string {
        return `label ${this.label} x${times}`;
    }
}
"#;

#[test]
fn typescript() {
    check(&[
        ("a.ts", TYPESCRIPT, "// A comment", Token::Comment),
        ("a.ts", TYPESCRIPT, "import", Token::Keyword),
        ("a.ts", TYPESCRIPT, "class", Token::Keyword),
        ("a.ts", TYPESCRIPT, "Widget", Token::Type),
        ("a.ts", TYPESCRIPT, "sealed", Token::Attribute),
        ("a.ts", TYPESCRIPT, "/^", Token::Regexp),
        ("a.ts", TYPESCRIPT, "node:fs", Token::String),
        ("a.ts", TYPESCRIPT, "this", Token::Builtin),
        // The reason `meta.template.expression` is in the table: without it
        // the whole template literal is one shade of green.
        ("a.ts", TYPESCRIPT, "${", Token::Escape),
    ]);
}

const PYTHON: &str = r#"# A comment.
import os


@dataclass
class Widget(Shape):
    label: str = "x"

    def render(self, times: int) -> str:
        value = self.label
        return f"{value} {times}"
"#;

#[test]
fn python() {
    check(&[
        ("a.py", PYTHON, "# A comment", Token::Comment),
        ("a.py", PYTHON, "import", Token::Keyword),
        ("a.py", PYTHON, "class Widget", Token::Keyword),
        ("a.py", PYTHON, "Widget", Token::Type),
        ("a.py", PYTHON, "Shape", Token::Type),
        ("a.py", PYTHON, "dataclass", Token::Attribute),
        ("a.py", PYTHON, "render", Token::Function),
        ("a.py", PYTHON, "self.label", Token::Builtin),
        ("a.py", PYTHON, "\"x\"", Token::String),
    ]);
}

const GO: &str = r#"// A comment.
package widget

import "fmt"

type Widget struct {
	Label string
}

func (w *Widget) Render(times int) string {
	return fmt.Sprintf("%s %d", w.Label, times)
}
"#;

#[test]
fn go() {
    check(&[
        ("a.go", GO, "// A comment", Token::Comment),
        ("a.go", GO, "package", Token::Keyword),
        ("a.go", GO, "type", Token::Keyword),
        ("a.go", GO, "Widget struct", Token::Type),
        ("a.go", GO, "struct {", Token::Keyword),
        ("a.go", GO, "Render", Token::Function),
        ("a.go", GO, "\"%s %d\"", Token::String),
    ]);
}

const C: &str = r#"/* A comment. */
#include <stdio.h>

struct widget {
    char label[64];
};

int render(int times) {
    char mark = '\t';
    printf("%d\n", times);
    return 0;
}
"#;

#[test]
fn c() {
    check(&[
        ("a.c", C, "/* A comment", Token::Comment),
        ("a.c", C, "include", Token::Keyword),
        ("a.c", C, "<stdio.h>", Token::String),
        ("a.c", C, "struct", Token::Keyword),
        // C's grammar calls `int` a type rather than a built-in type, so it
        // wears the type colour. Neovim shows exactly the same, for the same
        // reason: this is the grammar's judgement and we do not overrule it.
        // The matcher disagrees — see `the_two_engines_differ_where_the_grammars_do`.
        ("a.c", C, "int render", Token::Type),
        ("a.c", C, "render", Token::Function),
        ("a.c", C, "'\\t'", Token::Character),
        // Not `Escape`: nothing in C's grammar picks a format specifier out
        // of a string, where the matcher's `constant.other.placeholder` does.
        // A small, real loss, kept visible here rather than in a comment.
        ("a.c", C, "%d", Token::String),
    ]);
}

const MARKDOWN: &str = r#"# A heading

Some **bold** and *slanted* words, `inline code`, and
[a label](https://example.com) too.

> A quotation.

- a bullet
"#;

#[test]
fn markdown() {
    check(&[
        ("a.md", MARKDOWN, "# A heading", Token::Heading),
        ("a.md", MARKDOWN, "**bold**", Token::Emphasis),
        ("a.md", MARKDOWN, "*slanted*", Token::Emphasis),
        ("a.md", MARKDOWN, "`inline", Token::Raw),
        ("a.md", MARKDOWN, "https://", Token::Link),
        ("a.md", MARKDOWN, "> A quotation", Token::Quote),
        ("a.md", MARKDOWN, "- a bullet", Token::List),
    ]);
}

const JSON: &str = r#"{
  "label": "widget",
  "times": 3,
  "on": true
}
"#;

#[test]
fn json() {
    check(&[
        ("a.json", JSON, "\"label\"", Token::Property),
        ("a.json", JSON, "\"widget\"", Token::String),
        ("a.json", JSON, "3", Token::Constant),
        ("a.json", JSON, "true", Token::Constant),
    ]);
}

const YAML: &str = r#"# A comment.
defaults: &base
  retries: 3
widget:
  <<: *base
  label: "x"
"#;

#[test]
fn yaml() {
    check(&[
        ("a.yaml", YAML, "# A comment", Token::Comment),
        ("a.yaml", YAML, "defaults", Token::Property),
        ("a.yaml", YAML, "3", Token::Constant),
        ("a.yaml", YAML, "\"x\"", Token::String),
    ]);
}

const HTML: &str = r#"<!-- A comment. -->
<div class="page" id="main">
  <a href="https://example.com">text</a>
</div>
"#;

#[test]
fn html() {
    check(&[
        ("a.html", HTML, "<!-- A comment", Token::Comment),
        ("a.html", HTML, "div", Token::Tag),
        ("a.html", HTML, "class", Token::Attribute),
        // The quotes are not part of the value node, so the needle is the
        // value itself.
        ("a.html", HTML, "page", Token::String),
    ]);
}

const SHELL: &str = r#"#!/usr/bin/env bash
# A comment.
name="widget"

render() {
    printf '%s\n' "$name"
}
"#;

#[test]
fn shell() {
    check(&[
        ("build.sh", SHELL, "# A comment", Token::Comment),
        ("build.sh", SHELL, "\"widget\"", Token::String),
        ("build.sh", SHELL, "render", Token::Function),
        // Every command is a function to the shell's grammar; it has no
        // notion of a builtin. The matcher calls it `support.function`.
        ("build.sh", SHELL, "printf", Token::Function),
    ]);
}

#[test]
fn a_shebang_alone_is_enough_to_pick_a_grammar() {
    // No extension at all: the only clue is the first line, which is the
    // ordinary case for a script in `bin/`.
    assert_eq!(
        token_of("bin/release", SHELL, "# A comment"),
        Some(Token::Comment)
    );
}

#[test]
fn a_language_we_do_not_know_is_left_plain_rather_than_guessed() {
    let engine = Engine::new();
    assert!(engine.find(Clues::new("notes.qqzz", None), 1).is_none());
}

#[test]
fn a_language_with_a_parser_is_answered_by_the_parser() {
    // Which engine reads a file is the seam's decision and there is no way to
    // ask for the other one — a parser is preferred wherever there is one.
    // This pins the *consequence* rather than the mechanism: `int` here is C's
    // grammar's judgement, and if the file ever fell back to the matcher it
    // would say `Keyword` instead. See D39 and D41.
    assert_eq!(token_of("a.c", C, "int render"), Some(Token::Type));
}

#[test]
fn a_language_with_no_parser_still_reaches_the_matcher() {
    // The fallback, end to end: nothing here has a tree-sitter grammar, and
    // it is coloured anyway.
    assert_eq!(
        token_of("Makefile", "# a comment\nall:\n\techo hi\n", "# a comment"),
        Some(Token::Comment)
    );
}
