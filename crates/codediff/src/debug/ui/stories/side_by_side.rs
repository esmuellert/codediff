use anyhow::Result;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
#[cfg(test)]
use super::super::fixtures::MIN_LONG_LINE_CELLS;
use super::super::fixtures::diff::DiffFixture;
use super::super::fixtures::long_rust_line;

pub const STORIES: &[StoryDefinition] = &[
    story(
        "side-by-side/unchanged",
        "Unchanged lines mirrored across the divider",
        unchanged,
    ),
    story(
        "side-by-side/replacement",
        "One replacement with inner character ranges",
        replacement,
    ),
    story(
        "side-by-side/insert-delete",
        "Uneven insertion and deletion with filler rows",
        insert_delete,
    ),
    story(
        "side-by-side/tabs-unicode",
        "Tabs, CJK, emoji, and wide-cell alignment",
        tabs_unicode,
    ),
    story(
        "side-by-side/long-lines",
        "Long lines plus enough rows for two-axis scrolling",
        long_lines,
    ),
    StoryDefinition {
        id: "side-by-side/edge-matrix",
        description: "Many diff edge cases with a three-digit gutter",
        component: StoryComponent::SideBySide,
        snapshot_size: (120, 30),
        initial_keys: &[],
        make_fixture: edge_matrix,
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
        component: StoryComponent::SideBySide,
        snapshot_size: (100, 24),
        initial_keys: &[],
        make_fixture,
    }
}

fn unchanged() -> Result<StoryFixture> {
    Ok(StoryFixture::SideBySide(
        DiffFixture::from_lines(
            "unchanged.rs",
            &[
                "fn unchanged() {",
                "    println!(\"same on both sides\");",
                "}",
            ],
            &[
                "fn unchanged() {",
                "    println!(\"same on both sides\");",
                "}",
            ],
        )
        .build()?,
    ))
}

fn replacement() -> Result<StoryFixture> {
    Ok(StoryFixture::SideBySide(
        DiffFixture::from_lines(
            "replacement.rs",
            &["fn palette() {", "    let colour = \"blue\";", "}"],
            &["fn palette() {", "    let colour = \"green\";", "}"],
        )
        .build()?,
    ))
}

fn insert_delete() -> Result<StoryFixture> {
    Ok(StoryFixture::SideBySide(
        DiffFixture::from_lines(
            "insert-delete.rs",
            &[
                "fn update() {",
                "    // removed original line",
                "    shared();",
                "}",
            ],
            &[
                "fn update() {",
                "    // inserted modified line",
                "    // another inserted line",
                "    shared();",
                "}",
            ],
        )
        .build()?,
    ))
}

fn tabs_unicode() -> Result<StoryFixture> {
    Ok(StoryFixture::SideBySide(
        DiffFixture::from_lines(
            "tabs-unicode.rs",
            &["fn greet() {", "\tlet message = \"你好 👋🏽\";", "}"],
            &["fn greet() {", "\tlet message = \"您好 🦀\";", "}"],
        )
        .build()?,
    ))
}

fn long_lines() -> Result<StoryFixture> {
    let mut original = vec![long_rust_line("ORIGINAL_LONG_PREFIX", "0123456789abcdef")];
    let mut modified = vec![long_rust_line("MODIFIED_LONG_PREFIX", "fedcba9876543210")];
    for line in 2..=40 {
        original.push(format!(
            "// original row {line:02} with enough text to scroll"
        ));
        modified.push(format!(
            "// modified row {line:02} with enough text to scroll"
        ));
    }
    let original: Vec<&str> = original.iter().map(String::as_str).collect();
    let modified: Vec<&str> = modified.iter().map(String::as_str).collect();
    Ok(StoryFixture::SideBySide(
        DiffFixture::from_lines("long-lines.rs", &original, &modified).build()?,
    ))
}

fn edge_matrix() -> Result<StoryFixture> {
    let fixture = DiffFixture::from_text(
        "edge-matrix.rs",
        include_str!("../fixtures/data/edge_matrix/original.txt"),
        include_str!("../fixtures/data/edge_matrix/modified.txt"),
    )
    .with_line_pair(
        long_rust_line("ORIGINAL_EDGE_LONG_PREFIX", "original0123456789"),
        long_rust_line("MODIFIED_EDGE_LONG_PREFIX", "modified9876543210"),
    )
    .with_unchanged_lines("unchanged context line", 110);
    Ok(StoryFixture::SideBySide(fixture.build()?))
}

#[cfg(test)]
mod tests {
    use align::DiffVersion;

    use super::*;

    #[test]
    fn long_lines_exceed_a_wide_terminal() {
        let StoryFixture::SideBySide(content) = long_lines().unwrap() else {
            unreachable!()
        };
        let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
            unreachable!()
        };

        for version in [DiffVersion::Original, DiffVersion::Modified] {
            let line = &diff.alignment.lines(version)[0];
            let cells = line_index::LineIndex::new(line, line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get();
            assert!(
                cells >= MIN_LONG_LINE_CELLS,
                "{version:?} has only {cells} cells"
            );
        }
    }

    #[test]
    fn edge_matrix_has_edge_content_and_three_digit_lines() {
        let StoryFixture::SideBySide(content) = edge_matrix().unwrap() else {
            unreachable!()
        };
        let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
            unreachable!()
        };
        let original = diff.alignment.lines(DiffVersion::Original);
        let modified = diff.alignment.lines(DiffVersion::Modified);

        assert!(original.len() >= 100);
        assert!(modified.len() >= 100);
        assert!(original.iter().any(|line| line.contains("deleted only")));
        assert!(modified.iter().any(|line| line.contains("inserted only")));
        assert!(original.iter().any(|line| line.contains("你好")));
        for lines in [original, modified] {
            let long = lines
                .iter()
                .find(|line| line.contains("EDGE_LONG_PREFIX"))
                .expect("edge-matrix long line");
            let cells = line_index::LineIndex::new(long, line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get();
            assert!(cells >= MIN_LONG_LINE_CELLS);
        }
    }
}
