use anyhow::Result;

use super::super::definition::{StoryDefinition, StoryFixture, StoryType};
#[cfg(test)]
use super::super::fixtures::LONG_LINE_MIN_CELLS;
use super::super::fixtures::long_rust_constant;
use super::super::fixtures::single_file::{Presence, SingleFileFixture};

pub const STORIES: &[StoryDefinition] = &[
    story(
        "single-file/added",
        "An added file with no fake diff decorations",
        added,
    ),
    story(
        "single-file/deleted",
        "A deleted file shown from its only present side",
        deleted,
    ),
    story(
        "single-file/syntax",
        "A small Rust file after syntax colours arrive",
        syntax,
    ),
    story(
        "single-file/long-lines",
        "A full-width file with horizontal overflow",
        long_lines,
    ),
    story(
        "single-file/empty",
        "A present but empty one-sided file",
        empty,
    ),
    StoryDefinition {
        id: "single-file/large-syntax",
        summary: "Two hundred syntax-coloured lines for scrolling and chunking",
        story_type: StoryType::SingleFile,
        default_size: (100, 30),
        setup: &[],
        build: large_syntax,
    },
];

const fn story(
    id: &'static str,
    summary: &'static str,
    build: fn() -> Result<StoryFixture>,
) -> StoryDefinition {
    StoryDefinition {
        id,
        summary,
        story_type: StoryType::SingleFile,
        default_size: (100, 24),
        setup: &[],
        build,
    }
}

fn added() -> Result<StoryFixture> {
    Ok(single(SingleFileFixture::from_lines(
        "added.rs",
        Presence::Added,
        &[
            "pub fn newly_added() {",
            "    println!(\"added story\");",
            "}",
        ],
    )))
}

fn deleted() -> Result<StoryFixture> {
    Ok(single(SingleFileFixture::from_lines(
        "deleted.rs",
        Presence::Deleted,
        &[
            "pub fn removed_file() {",
            "    println!(\"deleted story\");",
            "}",
        ],
    )))
}

fn syntax() -> Result<StoryFixture> {
    Ok(single(SingleFileFixture::from_lines(
        "syntax.rs",
        Presence::Added,
        &[
            "fn highlighted() {",
            "    let answer: u32 = 42;",
            "    println!(\"{answer}\");",
            "}",
        ],
    )))
}

fn long_lines() -> Result<StoryFixture> {
    let long = long_rust_constant("SINGLE_LONG_PREFIX", "0123456789abcdef");
    Ok(single(SingleFileFixture::from_lines(
        "long-lines.rs",
        Presence::Added,
        &[long.as_str(), "// short tail"],
    )))
}

fn empty() -> Result<StoryFixture> {
    Ok(single(SingleFileFixture::empty(
        "empty.rs",
        Presence::Added,
    )))
}

fn large_syntax() -> Result<StoryFixture> {
    Ok(single(SingleFileFixture::generated_rust(
        "large-syntax.rs",
        200,
    )))
}

fn single(fixture: SingleFileFixture) -> StoryFixture {
    StoryFixture::SingleFile(fixture.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(fixture: StoryFixture) -> usize {
        let StoryFixture::SingleFile(content) = fixture else {
            unreachable!()
        };
        let pipeline::diff::DiffContent::SingleFile(single) = content.as_ref() else {
            unreachable!()
        };
        single.lines.len()
    }

    #[test]
    fn long_line_exceeds_a_wide_terminal() {
        let StoryFixture::SingleFile(content) = long_lines().unwrap() else {
            unreachable!()
        };
        let pipeline::diff::DiffContent::SingleFile(single) = content.as_ref() else {
            unreachable!()
        };
        let cells = line_index::LineIndex::new(&single.lines[0], line_index::DEFAULT_TAB_WIDTH)
            .width()
            .get();

        assert!(
            cells >= LONG_LINE_MIN_CELLS,
            "long line has only {cells} cells"
        );
    }

    #[test]
    fn edge_files_are_empty_and_large_respectively() {
        assert_eq!(lines(empty().unwrap()), 0);
        assert_eq!(lines(large_syntax().unwrap()), 200);
    }
}
