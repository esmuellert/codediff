//! Every language, checked word by word.
//!
//! One case per construct per language: the words a reader looks for first —
//! a keyword, a type, a function, a string, a comment — and the role each must
//! end up wearing. Twenty-five of these go through the parser and the rest
//! through the matcher, and **this file does not say which**. That is the
//! point: a reader does not care how the answer was reached, and a language
//! moving between engines is caught here by the answer changing rather than by
//! nothing at all.
//!
//! The narrower checks live elsewhere. `scopes.rs` proves every TextMate
//! selector claims something; this proves the right one wins, in every
//! language we claim to support.

use syntax::{Clues, Engine, Highlighted, Palette};
use ui::theme::Token;

/// The token covering the first occurrence of `needle`, through the real seam.
fn token_of(path: &str, source: &str, needle: &str) -> Option<Token> {
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
        .unwrap_or_else(|| panic!("nothing claims {path}"));
    let mut read = Highlighted::new(&engine, grammar, &palette, &lines);
    read.reach(&engine, &palette, lines.len() as u32, &lines);

    let (n, byte) = lines
        .iter()
        .enumerate()
        .find_map(|(n, line)| line.find(needle).map(|b| (n, b as u32)))
        .unwrap_or_else(|| panic!("{path}: {needle:?} is not in the sample"));

    read.line(n as u32)
        .iter()
        .find(|span| span.bytes.contains(&byte))
        .and_then(|span| span.style.pen)
        .and_then(ui::theme::token)
}

/// `(path, source, &[(word, token)])`.
type Language = (&'static str, &'static str, &'static [(&'static str, Token)]);

fn check(languages: &[Language]) {
    let mut wrong = Vec::new();
    for (path, source, cases) in languages {
        for (needle, want) in *cases {
            let got = token_of(path, source, needle);
            if got != Some(*want) {
                wrong.push(format!(
                    "{path:<16} {needle:<22} is {:<12} but should be {}",
                    got.map_or("nothing", Token::name),
                    want.name(),
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

// --- the languages a parser reads ------------------------------------------

const PARSED: &[Language] = &[
    (
        "a.rs",
        "// note\nstruct Widget { name: String }\nfn make(n: u32) -> Widget { let s = \"hi\"; }\n",
        &[
            ("// note", Token::Comment),
            ("struct", Token::Keyword),
            ("Widget {", Token::Type),
            ("make", Token::Function),
            ("n: u32", Token::Parameter),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.py",
        "# note\nimport os\n\n\nclass Widget(Base):\n    def make(self, n: int) -> str:\n        return \"hi\"\n",
        &[
            ("# note", Token::Comment),
            ("import", Token::Keyword),
            ("class", Token::Keyword),
            ("Widget(", Token::Type),
            ("make", Token::Function),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.js",
        "// note\nclass Widget {\n  make(n) { return `hi ${n}`; }\n}\nconst x = 1;\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("Widget", Token::Type),
            ("make", Token::Function),
            ("const", Token::Keyword),
            ("1;", Token::Constant),
        ],
    ),
    (
        "a.ts",
        "// note\ninterface Shape { n: number }\nclass Widget implements Shape {\n  make(n: number): string { return \"hi\"; }\n}\n",
        &[
            ("// note", Token::Comment),
            ("interface", Token::Keyword),
            ("Shape {", Token::Type),
            ("make", Token::Function),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.tsx",
        "// note\nconst App = () => <div className=\"page\">hi</div>;\n",
        &[("// note", Token::Comment), ("const", Token::Keyword)],
    ),
    (
        "a.go",
        "// note\npackage widget\n\ntype Widget struct { Name string }\n\nfunc (w *Widget) Make(n int) string { return \"hi\" }\n",
        &[
            ("// note", Token::Comment),
            ("package", Token::Keyword),
            ("Widget struct", Token::Type),
            ("Make", Token::Function),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.java",
        "// note\npublic class Widget extends Base {\n  private String name;\n  public String make(int n) { return \"hi\"; }\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("Widget", Token::Type),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.c",
        "/* note */\n#include <stdio.h>\nstruct widget { char name[8]; };\nint make(int n) { return 0; }\n",
        &[
            ("/* note */", Token::Comment),
            ("struct", Token::Keyword),
            ("make", Token::Function),
        ],
    ),
    (
        "a.cc",
        "// note\nnamespace w {\nclass Widget : public Base {\n public:\n  std::string make(int n) { return \"hi\"; }\n};\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.cs",
        "// note\nnamespace W {\n  public class Widget : Base {\n    public string Make(int n) => \"hi\";\n  }\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("Widget", Token::Type),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.rb",
        "# note\nmodule Greeting\n  class Widget < Base\n    def make(n)\n      \"hi\"\n    end\n  end\nend\n",
        &[
            ("# note", Token::Comment),
            ("module", Token::Keyword),
            ("def", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.php",
        "<?php\n// note\nclass Widget extends Base {\n  public function make(int $n): string { return \"hi\"; }\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "build.sh",
        "#!/usr/bin/env bash\n# note\nname=\"widget\"\nmake() {\n  printf '%s\\n' \"$name\"\n}\nif [[ -n \"$name\" ]]; then make; fi\n",
        &[
            ("# note", Token::Comment),
            ("\"widget\"", Token::String),
            ("if", Token::Keyword),
        ],
    ),
    (
        "a.json",
        "{\n  \"name\": \"widget\",\n  \"count\": 3,\n  \"on\": true\n}\n",
        &[
            ("\"name\"", Token::Property),
            ("\"widget\"", Token::String),
            ("3", Token::Constant),
            ("true", Token::Constant),
        ],
    ),
    (
        "a.yaml",
        "# note\nname: widget\ncount: 3\nlist:\n  - one\n",
        &[("# note", Token::Comment), ("count", Token::Property)],
    ),
    (
        "a.toml",
        "# note\n[package]\nname = \"widget\"\ncount = 3\n",
        &[
            ("# note", Token::Comment),
            ("\"widget\"", Token::String),
            ("3", Token::Constant),
        ],
    ),
    (
        "a.css",
        "/* note */\n.page > a:hover {\n  color: #cba6f7;\n  content: \"hi\";\n}\n",
        &[("/* note */", Token::Comment), ("\"hi\"", Token::String)],
    ),
    (
        "a.html",
        "<!-- note -->\n<div class=\"page\" id=\"main\">\n  <a href=\"http://x\">text</a>\n</div>\n",
        &[
            ("<!-- note -->", Token::Comment),
            ("div", Token::Tag),
            ("class", Token::Attribute),
            ("page", Token::String),
        ],
    ),
    (
        "a.lua",
        "-- note\nlocal Widget = {}\nfunction Widget.make(n)\n  return \"hi\"\nend\n",
        &[
            ("-- note", Token::Comment),
            ("local", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.scala",
        "// note\nclass Widget(name: String) extends Base {\n  def make(n: Int): String = \"hi\"\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.swift",
        "// note\nclass Widget: Base {\n  func make(n: Int) -> String { return \"hi\" }\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.hs",
        "-- note\nmodule Widget where\n\nmake :: Int -> String\nmake n = \"hi\"\n",
        &[("-- note", Token::Comment), ("\"hi\"", Token::String)],
    ),
    (
        "a.ex",
        "# note\ndefmodule Widget do\n  def make(n) do\n    \"hi\"\n  end\nend\n",
        &[
            ("# note", Token::Comment),
            ("def make", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.nix",
        "# note\n{ pkgs }:\nrec {\n  name = \"widget\";\n  count = 3;\n}\n",
        &[("# note", Token::Comment), ("\"widget\"", Token::String)],
    ),
    (
        "a.sql",
        "-- note\nSELECT name, COUNT(*) AS total\nFROM widgets\nWHERE name LIKE 'a%';\n",
        &[
            ("-- note", Token::Comment),
            ("SELECT", Token::Keyword),
            ("'a%'", Token::String),
        ],
    ),
];

// --- the languages the matcher reads ---------------------------------------

const MATCHED: &[Language] = &[
    (
        "a.md",
        "# Heading\n\nSome **bold** and `code` and a [label](http://x).\n\n- item\n",
        &[
            ("# Heading", Token::Heading),
            ("**bold**", Token::Emphasis),
            ("`code`", Token::Raw),
            ("http://x", Token::Link),
            ("- item", Token::List),
        ],
    ),
    (
        "a.kt",
        "// note\nclass Widget(val name: String) {\n  fun make(n: Int): String = \"hi\"\n}\n",
        &[
            ("// note", Token::Comment),
            ("class", Token::Keyword),
            ("\"hi\"", Token::String),
        ],
    ),
    (
        "a.dart",
        "// note\nclass Widget {\n  String make(int n) => \"hi\";\n}\n",
        &[("// note", Token::Comment), ("\"hi\"", Token::String)],
    ),
    (
        "a.pl",
        "# note\nsub make {\n  my $n = shift;\n  return \"hi\";\n}\n",
        &[("# note", Token::Comment), ("\"hi\"", Token::String)],
    ),
    (
        "Makefile",
        "# note\nall: build\n\nbuild:\n\techo hi\n",
        &[("# note", Token::Comment)],
    ),
    (
        "a.diff",
        "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n",
        &[("+new", Token::Inserted), ("-old", Token::Deleted)],
    ),
];

#[test]
fn every_parsed_language_colours_what_a_reader_looks_for() {
    check(PARSED);
}

#[test]
fn every_matched_language_colours_what_a_reader_looks_for() {
    check(MATCHED);
}

#[test]
fn every_sample_is_read_by_the_engine_it_is_filed_under() {
    // The two lists are not decoration: `PARSED` documents which languages we
    // ship a grammar for. A language quietly falling back — a crate dropped, a
    // detection table typo — would still colour, and nothing else here would
    // notice the loss.
    let engine = Engine::new();
    let engine_for = |path: &str, source: &str| {
        let first = source.lines().next();
        let lines = source.lines().count();
        let grammar = engine
            .find(Clues::new(path, first), lines)
            .unwrap_or_else(|| panic!("nothing claims {path}"));
        engine.name(grammar).to_owned()
    };
    // The parser names languages in lower case with underscores; the matcher
    // uses the grammar's display name. Not a rule worth relying on, so the
    // check is that the two sets do not overlap.
    for (path, source, _) in PARSED {
        let name = engine_for(path, source);
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{path} fell back to the matcher, as {name}"
        );
    }
    for (path, source, _) in MATCHED {
        let name = engine_for(path, source);
        assert!(
            name.chars()
                .any(|c| c.is_ascii_uppercase() || c == ' ' || c == '-'),
            "{path} is now parsed, as {name} — move it to PARSED"
        );
    }
}

#[test]
fn every_language_names_a_comment_a_comment() {
    // The one construct every language in both lists has, and the one most
    // likely to break when a query is composed wrongly: a file whose comments
    // are plain is a file whose query never ran.
    for (path, source, _) in PARSED.iter().chain(MATCHED) {
        // Two formats where the marker means something else: `#` starts a
        // heading in Markdown, and `---` starts a file header in a patch.
        // Neither has comments at all.
        if path.ends_with(".md") || path.ends_with(".diff") {
            continue;
        }
        let Some(line) = source.lines().find(|l| {
            let t = l.trim_start();
            t.starts_with("//") || t.starts_with('#') || t.starts_with("--") || t.starts_with("/*")
        }) else {
            continue;
        };
        let needle = line.trim_start();
        assert_eq!(
            token_of(path, source, needle),
            Some(Token::Comment),
            "{path}: {needle:?}"
        );
    }
}
