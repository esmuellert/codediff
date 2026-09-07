use anyhow::Result;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
#[cfg(test)]
use super::super::fixtures::MIN_LONG_LINE_CELLS;
use super::super::fixtures::diff::DiffFixture;
use super::super::fixtures::long_rust_line;

pub const STORIES: &[StoryDefinition] = &[
    story(
        "inline/unchanged",
        "Unchanged lines with both line numbers",
        unchanged,
    ),
    story(
        "inline/replacement",
        "Deleted text followed by its replacement",
        replacement,
    ),
    story(
        "inline/insert-delete",
        "Uneven deletions and insertions in one column",
        insert_delete,
    ),
    story(
        "inline/tabs-unicode",
        "Tabs, CJK, emoji, and wide-cell alignment",
        tabs_unicode,
    ),
    story(
        "inline/long-lines",
        "Long lines and rows for two-axis scrolling",
        long_lines,
    ),
    StoryDefinition {
        id: "inline/edge-matrix",
        description: "Many diff edge cases with two three-digit gutters",
        component: StoryComponent::Inline,
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
        component: StoryComponent::Inline,
        snapshot_size: (100, 24),
        initial_keys: &[],
        make_fixture,
    }
}

fn unchanged() -> Result<StoryFixture> {
    Ok(StoryFixture::Inline(
        DiffFixture::from_lines(
            "inline-unchanged.rs",
            &[
                "fn unchanged() {",
                "    println!(\"same in one column\");",
                "}",
            ],
            &[
                "fn unchanged() {",
                "    println!(\"same in one column\");",
                "}",
            ],
        )
        .build()?,
    ))
}

fn replacement() -> Result<StoryFixture> {
    Ok(StoryFixture::Inline(
        DiffFixture::from_lines(
            "inline-replacement.rs",
            &["fn palette() {", "    let colour = \"blue\";", "}"],
            &["fn palette() {", "    let colour = \"green\";", "}"],
        )
        .build()?,
    ))
}

fn insert_delete() -> Result<StoryFixture> {
    Ok(StoryFixture::Inline(
        DiffFixture::from_lines(
            "inline-insert-delete.rs",
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
    Ok(StoryFixture::Inline(
        DiffFixture::from_lines(
            "inline-tabs-unicode.rs",
            &["fn greet() {", "\tlet message = \"你好 👋🏽\";", "}"],
            &["fn greet() {", "\tlet message = \"您好 🦀\";", "}"],
        )
        .build()?,
    ))
}

fn long_lines() -> Result<StoryFixture> {
    let mut original = vec![long_rust_line(
        "INLINE_ORIGINAL_LONG_PREFIX",
        "0123456789abcdef",
    )];
    let mut modified = vec![long_rust_line(
        "INLINE_MODIFIED_LONG_PREFIX",
        "fedcba9876543210",
    )];
    for line in 2..=40 {
        let unchanged = format!("// inline row {line:02}");
        original.push(unchanged.clone());
        modified.push(unchanged);
    }
    let original: Vec<&str> = original.iter().map(String::as_str).collect();
    let modified: Vec<&str> = modified.iter().map(String::as_str).collect();
    Ok(StoryFixture::Inline(
        DiffFixture::from_lines("inline-long-lines.rs", &original, &modified).build()?,
    ))
}

fn edge_matrix() -> Result<StoryFixture> {
    let fixture = DiffFixture::from_text(
        "inline-edge-matrix.rs",
        include_str!("../fixtures/data/edge_matrix/original.txt"),
        include_str!("../fixtures/data/edge_matrix/modified.txt"),
    )
    .with_line_pair(
        long_rust_line("INLINE_ORIGINAL_EDGE_LONG_PREFIX", "original0123456789"),
        long_rust_line("INLINE_MODIFIED_EDGE_LONG_PREFIX", "modified9876543210"),
    )
    .with_unchanged_lines("inline unchanged context", 110);
    Ok(StoryFixture::Inline(fixture.build()?))
}

#[cfg(test)]
mod tests {
    use align::DiffVersion;

    use super::*;

    #[test]
    fn long_lines_exceed_a_wide_terminal() {
        let StoryFixture::Inline(content) = long_lines().unwrap() else {
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
}
