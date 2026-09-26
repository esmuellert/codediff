use anyhow::Result;
use file_types::Stats;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
use super::super::fixtures::diff::DiffFixture;
use super::super::fixtures::long_rust_line;

pub const STORIES: &[StoryDefinition] = &[StoryDefinition {
    id: "card/comprehensive",
    description: "A framed compact diff with status, statistics, folds, and edge content",
    component: StoryComponent::Card,
    compact: true,
    snapshot_size: (120, 30),
    initial_keys: &[],
    make_fixture: comprehensive,
}];

fn comprehensive() -> Result<StoryFixture> {
    let content = DiffFixture::compact("crates/ui/src/components/diff_card.rs")
        .with_line_pair(
            "\tlet greeting = \"你好 👋🏽\";",
            "\tlet greeting = \"您好 🦀\";",
        )
        .with_line_pair(
            long_rust_line("CARD_ORIGINAL_LONG_PREFIX", "0123456789abcdef"),
            long_rust_line("CARD_MODIFIED_LONG_PREFIX", "fedcba9876543210"),
        )
        .with_unchanged_lines("    // card context", 80)
        .with_stats(Stats::new(23, 17))
        .build()?;
    Ok(StoryFixture::Card(content))
}

#[cfg(test)]
mod tests {
    use align::DiffVersion;

    use super::*;

    #[test]
    fn comprehensive_card_fixture_covers_the_compact_cases() {
        let StoryFixture::Card(content) = comprehensive().unwrap() else {
            unreachable!()
        };
        let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
            unreachable!()
        };

        assert!(diff.alignment.lines(DiffVersion::Original).len() >= 400);
        assert!(diff.alignment.lines(DiffVersion::Modified).len() >= 400);
        assert!(diff.alignment.hunks().len() >= 6);
        assert_eq!(diff.file.get_stats(), Some(Stats::new(23, 17)));
        assert!(
            diff.alignment
                .lines(DiffVersion::Original)
                .iter()
                .any(|line| line.contains("你好"))
        );
        assert!(
            diff.alignment
                .lines(DiffVersion::Original)
                .iter()
                .any(|line| line.contains("CARD_ORIGINAL_LONG_PREFIX"))
        );
    }
}
