//! Tests multiline parser state across incremental reads.
//!
//! Highlighting starts at line 1 and retains state between lines.

use syntax::{Capture, Clues, Engine, Highlighted, Palette, Pen, Rule, Span, Style};

const STRING: Pen = Pen(1);
const COMMENT: Pen = Pen(2);
const KEYWORD: Pen = Pen(3);

fn palette() -> Palette {
    Palette::from_tables(
        &[
            Rule::new("string", Style::pen(STRING)),
            Rule::new("comment", Style::pen(COMMENT)),
            Rule::new("keyword", Style::pen(KEYWORD)),
            Rule::new("storage", Style::pen(KEYWORD)),
        ],
        &[
            Capture::new("string", Style::pen(STRING)),
            Capture::new("comment", Style::pen(COMMENT)),
            Capture::new("keyword", Style::pen(KEYWORD)),
        ],
    )
}

fn read(path: &str, source: &str) -> Vec<Vec<Span>> {
    let engine = Engine::new();
    let palette = palette();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let grammar = engine
        .find(Clues::new(path, None), lines.len())
        .expect("a grammar");
    let mut highlighted = Highlighted::new(&engine, grammar, &palette, &lines);
    let mut spans = Vec::new();
    highlighted.read_colours_to_line(&engine, &palette, lines.len() as u32, &lines, &mut spans);
    spans
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
    // The middle line inherits the opening comment state.
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
    // State must reset after the closing delimiter.
    let source = "/* one\n   two */\nfn after() {}\n";
    let spans = read("a.rs", source);
    assert_eq!(first(&spans, 2, source), Some(KEYWORD), "after the close");
}

#[test]
fn a_python_docstring_does_not_invert_the_rest_of_the_file() {
    // A docstring must not change the state of the following code.
    let source = "\
def f():
    \"\"\"
    a docstring
    \"\"\"
    return 1
";
    let spans = read("a.py", source);
    // A docstring is prose, not executable code.
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
    // Incremental reads must match one full read.
    let source = "fn a() {}\n/*\n comment\n*/\nfn b() {}\nfn c() {}\n";
    let engine = Engine::new();
    let palette = palette();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let grammar = engine
        .find(Clues::new("a.rs", None), lines.len())
        .expect("a grammar");

    let mut piecemeal = Highlighted::new(&engine, grammar, &palette, &lines);
    // Each call appends only newly read lines.
    let mut spans = Vec::new();
    for line in [0, 2, 5] {
        piecemeal.read_colours_to_line(&engine, &palette, line, &lines, &mut spans);
    }

    let whole = read("a.rs", source);
    assert_eq!(spans.len(), whole.len(), "no line read twice or missed");
    for (n, expected) in whole.iter().enumerate() {
        assert_eq!(&spans[n], expected, "line {n}");
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
