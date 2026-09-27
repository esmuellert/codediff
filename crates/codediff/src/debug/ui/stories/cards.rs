use std::rc::Rc;

use anyhow::Result;
use file_types::Stats;

use super::super::definition::{StoryComponent, StoryDefinition, StoryFixture};
use super::super::fixtures::diff::DiffFixture;

pub const STORIES: &[StoryDefinition] = &[StoryDefinition {
    id: "cards/comprehensive",
    description: "Several compact diffs in one vertically scrollable view",
    component: StoryComponent::Cards,
    compact: true,
    snapshot_size: (100, 24),
    initial_keys: &[],
    make_fixture: comprehensive,
}];

fn comprehensive() -> Result<StoryFixture> {
    let original_payload = format!(
        "    let original_payload = ORIGINAL_PAYLOAD_START_{}ORIGINAL_PAYLOAD_END;",
        "old_chunk_".repeat(24)
    );
    let modified_payload = format!(
        "    let modified_payload = MODIFIED_PAYLOAD_START_{}MODIFIED_PAYLOAD_END;",
        "new_chunk_".repeat(32)
    );
    let config_before = format!(
        "const ORIGINAL_CONFIG = ORIGINAL_CONFIG_START_{}ORIGINAL_CONFIG_END;",
        "before_field_".repeat(20)
    );
    let config_after = format!(
        "const MODIFIED_CONFIG = MODIFIED_CONFIG_START_{}MODIFIED_CONFIG_END;",
        "after_field_".repeat(26)
    );
    let cards = vec![
        DiffFixture::from_lines(
            "src/alpha.rs",
            &[
                "fn alpha() {",
                "    let old_value = 1;",
                "    let old_name = \"alpha\";",
                original_payload.as_str(),
                "}",
            ],
            &[
                "fn alpha() {",
                "    let new_value = 2;",
                "    let new_name = \"alpha updated\";",
                modified_payload.as_str(),
                "}",
            ],
        )
        .with_stats(Stats::new(3, 3))
        .build()?,
        DiffFixture::from_lines(
            "src/beta.rs",
            &["fn beta() {", "    let before = true;", "}"],
            &[
                "fn beta() {",
                "    let after = false;",
                "    let extra = \"你好\";",
                "}",
            ],
        )
        .with_stats(Stats::new(2, 1))
        .build()?,
        DiffFixture::from_lines(
            "src/delta.rs",
            &["pub fn delta() -> u32 {", "    10", "}"],
            &["pub fn delta() -> u32 {", "    20", "}"],
        )
        .with_stats(Stats::new(1, 1))
        .build()?,
        DiffFixture::from_lines(
            "src/epsilon.rs",
            &["let greeting = \"你好\";", "let flag = false;"],
            &["let greeting = \"您好\";", "let flag = true;"],
        )
        .with_stats(Stats::new(2, 2))
        .build()?,
        DiffFixture::from_lines(
            "src/zeta.rs",
            &[config_before.as_str()],
            &[config_after.as_str()],
        )
        .with_stats(Stats::new(1, 1))
        .build()?,
        DiffFixture::from_lines(
            "src/eta.rs",
            &["fn eta() {", "    println!(\"old output\");", "}"],
            &["fn eta() {", "    println!(\"new output\");", "}"],
        )
        .with_stats(Stats::new(1, 1))
        .build()?,
        DiffFixture::from_lines(
            "src/theta.rs",
            &["enum State { Old, Stable }"],
            &["enum State { New, Stable, Ready }"],
        )
        .with_stats(Stats::new(1, 1))
        .build()?,
        DiffFixture::from_lines(
            "src/gamma.rs",
            &["const OLD: &str = \"long original value\";"],
            &["const NEW: &str = \"long modified value\";"],
        )
        .with_stats(Stats::new(1, 1))
        .build()?,
    ];
    Ok(StoryFixture::Cards(Rc::new(cards)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comprehensive_cards_have_stable_order_and_stats() {
        let StoryFixture::Cards(cards) = comprehensive().unwrap() else {
            unreachable!()
        };
        assert_eq!(cards.len(), 8);
        assert_eq!(cards[0].file().path().as_str(), "src/alpha.rs");
        assert_eq!(cards[1].file().get_stats(), Some(Stats::new(2, 1)));
        assert_eq!(cards[7].file().path().as_str(), "src/gamma.rs");
        let original = cards[0]
            .alignment()
            .unwrap()
            .lines(align::DiffVersion::Original);
        let modified = cards[0]
            .alignment()
            .unwrap()
            .lines(align::DiffVersion::Modified);
        assert!(
            original
                .iter()
                .any(|line| line.contains("ORIGINAL_PAYLOAD_END"))
        );
        assert!(
            modified
                .iter()
                .any(|line| line.contains("MODIFIED_PAYLOAD_END"))
        );
        assert!(modified.iter().any(|line| line.chars().count() > 300));
        let zeta = cards[4]
            .alignment()
            .unwrap()
            .lines(align::DiffVersion::Modified);
        assert!(zeta.iter().any(|line| line.contains("MODIFIED_CONFIG_END")));
        assert!(zeta.iter().any(|line| line.chars().count() > 300));
    }
}
