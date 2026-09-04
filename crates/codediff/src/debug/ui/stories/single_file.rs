use anyhow::Result;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
#[cfg(test)]
use super::super::fixtures::MIN_LONG_LINE_CELLS;
use super::super::fixtures::long_rust_line;
use super::super::fixtures::single_file::{FilePresence, SingleFileFixture};

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
        "single-file/rust-syntax",
        "A small Rust file after syntax colours arrive",
        rust_syntax,
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
        id: "single-file/long-syntax-file",
        description: "Two hundred syntax-coloured lines for scrolling and chunking",
        component: StoryComponent::SingleFile,
        snapshot_size: (100, 30),
        initial_keys: &[],
        make_fixture: long_syntax_file,
    },
];

const fn story(
    id: &'static str,
    description: &'static str,
    make_fixture: fn() -> Result<StoryFixture>,
) -> StoryDefinition {
    StoryDefinition {
        id,
        description,
        component: StoryComponent::SingleFile,
        snapshot_size: (100, 24),
        initial_keys: &[],
        make_fixture,
    }
}

fn added() -> Result<StoryFixture> {
    Ok(single_file(SingleFileFixture::from_lines(
        "added.rs",
        FilePresence::Added,
        &[
            "pub fn newly_added() {",
            "    println!(\"added story\");",
            "}",
        ],
    )))
}

fn deleted() -> Result<StoryFixture> {
    Ok(single_file(SingleFileFixture::from_lines(
        "deleted.rs",
        FilePresence::Deleted,
        &[
            "pub fn removed_file() {",
            "    println!(\"deleted story\");",
            "}",
        ],
    )))
}

fn rust_syntax() -> Result<StoryFixture> {
    Ok(single_file(SingleFileFixture::from_lines(
        "syntax.rs",
        FilePresence::Added,
        &[
            "fn highlighted() {",
            "    let answer: u32 = 42;",
            "    println!(\"{answer}\");",
            "}",
        ],
    )))
}

fn long_lines() -> Result<StoryFixture> {
    let long = long_rust_line("SINGLE_LONG_PREFIX", "0123456789abcdef");
    Ok(single_file(SingleFileFixture::from_lines(
        "long-lines.rs",
        FilePresence::Added,
        &[long.as_str(), "// short tail"],
    )))
}

fn empty() -> Result<StoryFixture> {
    Ok(single_file(SingleFileFixture::empty(
        "empty.rs",
        FilePresence::Added,
    )))
}

fn long_syntax_file() -> Result<StoryFixture> {
    Ok(single_file(SingleFileFixture::generated_rust(
        "long-syntax-file.rs",
        200,
    )))
}

fn single_file(fixture: SingleFileFixture) -> StoryFixture {
    StoryFixture::SingleFile(fixture.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_count(fixture: StoryFixture) -> usize {
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
            cells >= MIN_LONG_LINE_CELLS,
            "long line has only {cells} cells"
        );
    }

    #[test]
    fn edge_files_are_empty_and_large_respectively() {
        assert_eq!(line_count(empty().unwrap()), 0);
        assert_eq!(line_count(long_syntax_file().unwrap()), 200);
    }
}
