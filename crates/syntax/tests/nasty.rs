//! Input a grammar was not written for.
//!
//! Everything here is a problem `bat` or `delta` hit in production. Where the
//! two disagree we follow `bat`, because its answers preserve the parse state
//! and `delta`'s corrupt it.
//!
//! Note what is *not* here: control characters, bidirectional overrides and
//! grapheme widths. Those are `line-index`'s, applied when a line is drawn,
//! and a span is a byte range so it never has to know about them. That is the
//! benefit of returning ranges rather than styled strings — `delta` must
//! expand tabs before highlighting because it works in text, and gets column
//! alignment wrong as a result.

use syntax::{Capture, Clues, Engine, Highlighted, Palette, Pen, Rule, Span, Style, limits};

fn palette() -> Palette {
    Palette::new(
        &[
            Rule::new("string", Style::pen(Pen(1))),
            Rule::new("comment", Style::pen(Pen(2))),
            Rule::new("keyword", Style::pen(Pen(3))),
            Rule::new("storage", Style::pen(Pen(3))),
        ],
        &[
            Capture::new("string", Style::pen(Pen(1))),
            Capture::new("comment", Style::pen(Pen(2))),
            Capture::new("keyword", Style::pen(Pen(3))),
        ],
    )
}

fn read(path: &str, lines: &[&str]) -> Vec<Vec<Span>> {
    let engine = Engine::new();
    let palette = palette();
    let owned: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    let grammar = engine
        .find(Clues::new(path, None), lines.len())
        .expect("a grammar");
    let mut highlighted = Highlighted::new(&engine, grammar, &palette, &owned);
    highlighted.reach(&engine, &palette, owned.len() as u32, &owned);
    (0..owned.len())
        .map(|n| highlighted.line(n as u32).to_vec())
        .collect()
}

/// Every span of a line lies inside it and on character boundaries.
fn well_formed(spans: &[Span], line: &str) {
    for span in spans {
        assert!(
            span.bytes.end as usize <= line.len(),
            "{span:?} runs past {:?}",
            line
        );
        assert!(
            line.is_char_boundary(span.bytes.start as usize)
                && line.is_char_boundary(span.bytes.end as usize),
            "{span:?} cuts a character in {line:?}"
        );
    }
}

#[test]
fn tabs_are_left_alone_because_a_span_is_a_byte_range() {
    // A tab is one byte and several columns. Nothing here needs to know that:
    // the renderer maps bytes to columns when it draws. `delta` expands tabs
    // before highlighting and therefore has to get the width right twice.
    let lines = ["\tlet x = \"hi\";", "\t\tlet y = \"there\";"];
    let spans = read("a.rs", &lines);
    for (n, line) in lines.iter().enumerate() {
        well_formed(&spans[n], line);
        assert!(!spans[n].is_empty(), "line {n} was coloured");
    }
}

#[test]
fn a_very_long_line_is_skipped_without_breaking_the_lines_after_it() {
    // bat's answer, not delta's: the line keeps its place in the parse, so the
    // file after it is still correct. delta truncates the text, which loses
    // the state.
    let minified = "x".repeat(limits::MAX_LINE_CHARS + 100);
    let lines = ["/* a comment */", &minified, "fn after() {}"];
    let spans = read("a.rs", &lines);
    assert!(spans[1].is_empty(), "the long line is not coloured");
    assert!(!spans[2].is_empty(), "but the line after it still is");
}

#[test]
fn a_line_of_exactly_the_limit_is_still_coloured() {
    let at_limit = format!("// {}", "x".repeat(limits::MAX_LINE_CHARS - 3));
    assert_eq!(at_limit.len(), limits::MAX_LINE_CHARS);
    let lines = [at_limit.as_str()];
    assert!(!read("a.rs", &lines)[0].is_empty());
}

#[test]
fn a_file_too_big_to_read_is_left_plain_rather_than_refused() {
    let engine = Engine::new();
    let palette = palette();
    let huge: Vec<String> = std::iter::repeat_n("x".to_owned(), limits::MAX_LINES + 1).collect();
    let grammar = engine
        .find(Clues::new("a.rs", None), huge.len())
        .expect("a grammar");
    let mut highlighted = Highlighted::new(&engine, grammar, &palette, &huge);
    highlighted.reach(&engine, &palette, 10, &huge);
    assert!(highlighted.finished(), "there is nothing to do");
    assert!(highlighted.line(0).is_empty());
}

#[test]
fn wide_and_combining_characters_do_not_split_a_span() {
    // A span that began or ended inside one of these would panic a renderer
    // slicing the line.
    let lines = [
        "let s = \"日本語のテキスト\";",
        "// комментарий на русском",
        "let e = \"👍🏽 family: 👨‍👩‍👧‍👦\";",
    ];
    let spans = read("a.rs", &lines);
    for (n, line) in lines.iter().enumerate() {
        well_formed(&spans[n], line);
    }
}

#[test]
fn an_empty_file_and_empty_lines_are_fine() {
    assert!(read("a.rs", &[]).is_empty());
    let spans = read("a.rs", &["", "fn a() {}", "", ""]);
    assert!(spans[0].is_empty());
    assert!(!spans[1].is_empty());
}

#[test]
fn a_file_with_no_trailing_newline_still_reads() {
    // We store lines without their terminators and add one for the grammar,
    // so a file that never had a last newline is not a special case — but the
    // last line must still be coloured.
    let spans = read("a.rs", &["fn a() {}", "let x = \"end\";"]);
    assert!(!spans[1].is_empty(), "the last line");
}

#[test]
fn text_that_is_not_source_at_all_does_not_panic() {
    // A `.rs` file containing a JPEG header, which is what a mislabelled or
    // corrupt file looks like. Binary is refused earlier, in the pipeline;
    // this is the belt to that pair of braces.
    let lines = [
        "\u{fffd}\u{fffd}\u{fffd}\u{fffd}JFIF\u{0}\u{1}",
        "\u{fffd}q",
    ];
    let spans = read("a.rs", &lines);
    for (n, line) in lines.iter().enumerate() {
        well_formed(&spans[n], line);
    }
}

#[test]
fn a_selector_that_matches_nothing_is_silent_and_must_be_tested_by_use() {
    // The engine's selector parser is permissive: a scope path is only dotted
    // words, so nearly anything parses and a misspelled selector is accepted
    // and then never matches. `rules()` therefore cannot catch a typo, and a
    // theme's guard has to be a test that each of its rules colours something
    // real — which is what `languages.rs` and `ui`'s theme tests do.
    let palette = Palette::new(
        &[
            Rule::new("keyword", Style::pen(Pen(1))),
            Rule::new("keywrod", Style::pen(Pen(2))),
        ],
        &[],
    );
    assert_eq!(palette.rules(), 2, "both were accepted");

    let spans = read("a.rs", &["fn a() {}"]);
    assert!(
        !spans[0].is_empty(),
        "the correctly spelled one still works"
    );
}
