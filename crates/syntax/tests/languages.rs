//! Every language we claim to support, checked on real code.
//!
//! The S11 criterion is *"keywords, strings and comments are coloured
//! correctly in all twelve"*, so that is what this asserts — by the pen that
//! reaches the caller, not by scope name, because a scope name that stopped
//! reaching the theme would still look right in a test that only checked
//! scopes.
//!
//! Four pens, deliberately distinct, so a mix-up is a failure rather than a
//! coincidence.

use syntax::{Capture, Clues, Engine, Highlighted, Palette, Pen, Rule, Span, Style};

const KEYWORD: Pen = Pen(1);
const STRING: Pen = Pen(2);
const COMMENT: Pen = Pen(3);
const MARKUP: Pen = Pen(4);

/// The smallest theme that can tell the four apart.
///
/// Both tables, with the same pens. Which engine reads a file is the
/// seam's business — a parser where we have a grammar, the matcher where we do
/// not — and these assertions are about the language, not the engine. Giving
/// both the same pens is what lets one test hold either way, and it is also
/// how a language that changes engines is caught: the answer must not move.
fn palette() -> Palette {
    Palette::from_tables(
        &[
            Rule::new("keyword", Style::pen(KEYWORD)),
            Rule::new("storage", Style::pen(KEYWORD)),
            Rule::new("string", Style::pen(STRING)),
            Rule::new("comment", Style::pen(COMMENT).italic()),
            // Markup, so the two prose formats have something to claim.
            Rule::new("markup", Style::pen(MARKUP)),
            Rule::new("entity.name", Style::pen(MARKUP)),
        ],
        &[
            Capture::new("keyword", Style::pen(KEYWORD)),
            Capture::new("type.builtin", Style::pen(KEYWORD)),
            Capture::new("string", Style::pen(STRING)),
            Capture::new("comment", Style::pen(COMMENT).italic()),
            Capture::new("type", Style::pen(MARKUP)),
            Capture::new("function", Style::pen(MARKUP)),
            Capture::new("property", Style::pen(MARKUP)),
        ],
    )
}

/// Every line of a file, coloured.
fn read(path: &str, source: &str) -> Vec<Vec<Span>> {
    let engine = Engine::new();
    let palette = palette();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let first = lines.first().map(String::as_str);
    let grammar = engine
        .find(Clues::new(path, first), lines.len())
        .unwrap_or_else(|| panic!("no grammar claims {path}"));
    let mut highlighted = Highlighted::new(&engine, grammar, &palette, &lines);
    let mut spans = Vec::new();
    highlighted.read_colours_to_line(&engine, &palette, lines.len() as u32, &lines, &mut spans);
    spans
}

/// Whether any span of the file uses the given pen.
fn has(spans: &[Vec<Span>], pen: Pen) -> bool {
    spans
        .iter()
        .flatten()
        .any(|span| span.style.pen == Some(pen))
}

/// The pen covering a byte of a line, if any.
fn at(spans: &[Vec<Span>], line: usize, byte: u32) -> Option<Pen> {
    spans
        .get(line)?
        .iter()
        .find(|span| span.bytes.contains(&byte))
        .and_then(|span| span.style.pen)
}

/// `(name, path, source)` for each language, with a keyword, a string and a
/// comment in every one.
const LANGUAGES: &[(&str, &str, &str)] = &[
    (
        "Rust",
        "src/main.rs",
        "// a comment\nfn main() {\n    let greeting = \"hello\";\n}\n",
    ),
    (
        "TypeScript",
        "src/app.ts",
        "// a comment\nfunction main(): void {\n  const greeting: string = \"hello\";\n}\n",
    ),
    (
        "JavaScript",
        "src/app.js",
        "// a comment\nfunction main() {\n  const greeting = \"hello\";\n}\n",
    ),
    (
        "Python",
        "app.py",
        "# a comment\ndef main():\n    greeting = \"hello\"\n    return greeting\n",
    ),
    (
        "Go",
        "main.go",
        "// a comment\npackage main\n\nfunc main() {\n\tgreeting := \"hello\"\n}\n",
    ),
    (
        "Java",
        "Main.java",
        "// a comment\nclass Main {\n  static String greeting = \"hello\";\n}\n",
    ),
    (
        "C",
        "main.c",
        "/* a comment */\n#include <stdio.h>\nint main(void) {\n  const char *g = \"hello\";\n  return 0;\n}\n",
    ),
    (
        "C++",
        "main.cpp",
        "// a comment\n#include <string>\nint main() {\n  const std::string g = \"hello\";\n  return 0;\n}\n",
    ),
    (
        "JSON",
        "package.json",
        "{\n  \"name\": \"codediff\",\n  \"version\": \"1.0.0\"\n}\n",
    ),
    (
        "YAML",
        "config.yaml",
        "# a comment\nname: codediff\nitems:\n  - \"hello\"\n",
    ),
    (
        "Markdown",
        "README.md",
        "# A heading\n\nSome text with `code` and a [link](http://example.com).\n",
    ),
    (
        "Bash",
        "deploy.sh",
        "#!/usr/bin/env bash\n# a comment\nif [ -n \"$1\" ]; then\n  echo \"hello\"\nfi\n",
    ),
    (
        "TOML",
        "Cargo.toml",
        "# a comment\n[package]\nname = \"codediff\"\n",
    ),
    (
        "Ruby",
        "app.rb",
        "# a comment\ndef main\n  greeting = \"hello\"\nend\n",
    ),
];

#[test]
fn every_language_is_recognised() {
    let engine = Engine::new();
    for (name, path, source) in LANGUAGES {
        let first = source.lines().next();
        let grammar = engine
            .find(Clues::new(path, first), source.lines().count())
            .unwrap_or_else(|| panic!("{name}: nothing claims {path}"));
        // Not asserting the exact grammar name — engines spell them
        // differently — only that something answered.
        assert!(!engine.name(grammar).is_empty(), "{name}");
    }
}

#[test]
fn every_language_colours_its_strings() {
    for (name, path, source) in LANGUAGES {
        // Markdown has no string literals; its quoted text is prose.
        if *name == "Markdown" {
            continue;
        }
        let spans = read(path, source);
        assert!(has(&spans, STRING), "{name}: no string was coloured");
    }
}

#[test]
fn every_language_colours_something() {
    // The catch-all, so that a language exempted from the three tests above
    // cannot silently come back with no colour at all.
    for (name, path, source) in LANGUAGES {
        let spans = read(path, source);
        assert!(
            spans.iter().any(|line| !line.is_empty()),
            "{name}: the whole file came back plain"
        );
    }
}

#[test]
fn every_language_with_comments_colours_them() {
    for (name, path, source) in LANGUAGES {
        // JSON has no comments, and Markdown's `#` is a heading.
        if matches!(*name, "JSON" | "Markdown") {
            continue;
        }
        let spans = read(path, source);
        assert!(has(&spans, COMMENT), "{name}: no comment was coloured");
    }
}

#[test]
fn every_programming_language_colours_its_keywords() {
    for (name, path, source) in LANGUAGES {
        // The four data and markup formats have no keywords to speak of.
        if matches!(*name, "JSON" | "YAML" | "Markdown" | "TOML") {
            continue;
        }
        let spans = read(path, source);
        assert!(has(&spans, KEYWORD), "{name}: no keyword was coloured");
    }
}

#[test]
fn the_three_kinds_land_on_the_right_characters() {
    // Colour by colour on one known file, so that "something was coloured"
    // cannot pass by colouring the wrong thing.
    let source = "// a comment\nfn main() {\n    let g = \"hello\";\n}\n";
    let spans = read("src/main.rs", source);

    assert_eq!(at(&spans, 0, 0), Some(COMMENT), "the `//`");
    assert_eq!(at(&spans, 1, 0), Some(KEYWORD), "the `fn`");
    // Not merely "coloured": a different colour from the keyword beside it,
    // which is the whole point of matching a scope path rather than a
    // category — `entity.name.function` is not `keyword`.
    assert_eq!(at(&spans, 1, 3), Some(MARKUP), "the function name");
    assert_ne!(at(&spans, 1, 3), at(&spans, 1, 0), "fn vs main");
    let quote = source.lines().nth(2).unwrap().find('"').unwrap() as u32;
    assert_eq!(at(&spans, 2, quote), Some(STRING), "the opening quote");
    assert_eq!(at(&spans, 2, 4), Some(KEYWORD), "the `let`");
}

#[test]
fn a_comment_carries_its_font_style_as_well_as_its_colour() {
    // A theme sets italic and colour independently; losing the flag would
    // still leave the colour right, so it needs its own assertion.
    let spans = read("src/main.rs", "// a comment\nfn main() {}\n");
    let comment = spans[0].first().expect("the comment is coloured");
    assert!(comment.style.italic, "{:?}", comment.style);
    assert!(!spans[1][0].style.italic, "code is not italic");
}

#[test]
fn an_unrecognised_file_is_plain_rather_than_a_failure() {
    let engine = Engine::new();
    assert!(
        engine
            .find(
                Clues::new("mystery.qqqqq", Some("nothing recognises this")),
                1
            )
            .is_none()
    );
    // And the caller's answer for such a file is "no spans", not a panic.
    let nothing = Highlighted::none();
    assert_eq!(nothing.get_lines_coloured(), 0);
    assert!(nothing.finished());
}

#[test]
fn a_shebang_names_a_language_when_the_name_cannot() {
    // The commonest case a diff viewer meets: an executable with no extension.
    let engine = Engine::new();
    let grammar = engine
        .find(Clues::new("scripts/deploy", Some("#!/usr/bin/env bash")), 1)
        .expect("a shebang is enough");
    assert!(
        engine.name(grammar).to_lowercase().contains("bash")
            || engine.name(grammar).to_lowercase().contains("shell"),
        "{}",
        engine.name(grammar)
    );
}

#[test]
fn spans_stay_inside_their_line() {
    // The engine is fed a newline we added; a span reaching past the caller's
    // line would slice out of bounds in a renderer.
    for (name, path, source) in LANGUAGES {
        let lines: Vec<&str> = source.lines().collect();
        for (n, spans) in read(path, source).iter().enumerate() {
            for span in spans {
                assert!(
                    span.bytes.end as usize <= lines[n].len(),
                    "{name} line {n}: {span:?} past {:?}",
                    lines[n]
                );
                assert!(
                    lines[n].is_char_boundary(span.bytes.start as usize),
                    "{name} line {n}: {span:?} starts mid-character"
                );
            }
        }
    }
}

#[test]
fn spans_are_in_order_and_do_not_overlap() {
    for (name, path, source) in LANGUAGES {
        for (n, spans) in read(path, source).iter().enumerate() {
            for pair in spans.windows(2) {
                assert!(
                    pair[0].bytes.end <= pair[1].bytes.start,
                    "{name} line {n}: {:?} overlaps {:?}",
                    pair[0],
                    pair[1]
                );
            }
        }
    }
}
