//! What reading a file from the top buys, and what a hunk viewer cannot have.
//!
//! `delta` starts its highlighter afresh at every hunk header, so a hunk that
//! begins inside a block comment or a multi-line string is coloured as though
//! it began at the top of the file — code shown as string, string shown as
//! code. It has been open since 2020 as
//! [#117](https://github.com/dandavison/delta/issues/117), and the proposed
//! fix is to ask git for `-U9999` and throw most of the answer away.
//!
//! We read whole snapshots, so the fix is free. These tests hold it that way:
//! they are the ones that would fail if anybody made the highlighter stateless
//! per line, or started it anywhere but line 1.

use syntax::{Clues, Engine, Highlighted, Palette, Pen, Rule, Span, Style};

const STRING: Pen = Pen(1);
const COMMENT: Pen = Pen(2);
const KEYWORD: Pen = Pen(3);

fn palette() -> Palette {
    Palette::new(&[
        Rule::new("string", Style::pen(STRING)),
        Rule::new("comment", Style::pen(COMMENT)),
        Rule::new("keyword", Style::pen(KEYWORD)),
        Rule::new("storage", Style::pen(KEYWORD)),
    ])
}

fn read(path: &str, source: &str) -> Vec<Vec<Span>> {
    let engine = Engine::new();
    let palette = palette();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let grammar = engine.find(Clues::new(path, None)).expect("a grammar");
    let mut highlighted = Highlighted::new(&engine, grammar, &palette, &lines);
    highlighted.reach(&engine, &palette, lines.len() as u32, &lines);
    (0..lines.len())
        .map(|n| highlighted.line(n as u32).to_vec())
        .collect()
}

/// The colour covering the first non-space character of a line.
fn first(spans: &[Vec<Span>], line: usize, source: &str) -> Option<Pen> {
    let text = source.lines().nth(line)?;
    let at = text.len() - text.trim_start().len();
    spans
        .get(line)?
        .iter()
        .find(|span| span.bytes.contains(&(at as u32)))
        .and_then(|span| span.style.pen)
}

#[test]
fn a_line_inside_a_block_comment_is_a_comment() {
    // The middle line has no comment marker of its own. Only the lines above
    // it say it is one.
    let source = "\
fn a() {}
/*
    this line is inside the comment
*/
fn b() {}
";
    let spans = read("a.rs", source);
    assert_eq!(first(&spans, 0, source), Some(KEYWORD), "before");
    assert_eq!(first(&spans, 2, source), Some(COMMENT), "inside");
    assert_eq!(first(&spans, 4, source), Some(KEYWORD), "after");
}

#[test]
fn code_after_a_block_comment_is_not_still_a_comment() {
    // The other half of the same bug: a highlighter that never saw the `*/`
    // would colour the rest of the file grey.
    let source = "/* one\n   two */\nfn after() {}\n";
    let spans = read("a.rs", source);
    assert_eq!(first(&spans, 2, source), Some(KEYWORD), "after the close");
}

#[test]
fn a_python_docstring_does_not_invert_the_rest_of_the_file() {
    // delta's own regression test, in their words: starting cold at the
    // closing `"""` makes it read as an *opening* one, so the docstring gets
    // code colours and the code gets string colours.
    let source = "\
def f():
    \"\"\"
    a docstring
    \"\"\"
    return 1
";
    let spans = read("a.py", source);
    // The grammar scopes a docstring as documentation rather than as a plain
    // string, so it lands on the comment rule — which is what most themes
    // want, and either way it is not code.
    let docstring = first(&spans, 2, source);
    assert!(
        docstring == Some(COMMENT) || docstring == Some(STRING),
        "the docstring is prose, not code: {docstring:?}"
    );
    assert_eq!(first(&spans, 4, source), Some(KEYWORD), "the return");
}

#[test]
fn a_multiline_string_holds_its_colour_across_lines() {
    let source = "const S: &str = \"one\n  two\n  three\";\nfn after() {}\n";
    let spans = read("a.rs", source);
    assert_eq!(first(&spans, 1, source), Some(STRING), "the middle");
    assert_eq!(first(&spans, 3, source), Some(KEYWORD), "after it closes");
}

#[test]
fn reading_lazily_gives_the_same_answer_as_reading_it_all() {
    // The laziness must be invisible. Reading a prefix, then more, then the
    // rest must land exactly where one pass would have.
    let source = "fn a() {}\n/*\n comment\n*/\nfn b() {}\nfn c() {}\n";
    let engine = Engine::new();
    let palette = palette();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let grammar = engine.find(Clues::new("a.rs", None)).expect("a grammar");

    let mut piecemeal = Highlighted::new(&engine, grammar, &palette, &lines);
    piecemeal.reach(&engine, &palette, 0, &lines);
    piecemeal.reach(&engine, &palette, 2, &lines);
    piecemeal.reach(&engine, &palette, 5, &lines);

    let whole = read("a.rs", source);
    for (n, expected) in whole.iter().enumerate() {
        assert_eq!(piecemeal.line(n as u32), expected, "line {n}");
    }
}

#[test]
fn a_hunk_read_on_its_own_would_have_been_wrong() {
    // The control for all of the above: start at what a hunk viewer would see
    // and the answer is different. If this ever stops differing, the tests
    // above have stopped proving anything.
    let inside_a_comment = "    this line is inside the comment\n*/\nfn b() {}\n";
    let spans = read("a.rs", inside_a_comment);
    assert_ne!(
        first(&spans, 0, inside_a_comment),
        Some(COMMENT),
        "without the lines above it, this cannot be known to be a comment"
    );
}
