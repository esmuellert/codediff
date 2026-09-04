//! `codediff debug ui` — deterministic production-component stories.

mod catalog;
mod catalog_rows;
mod catalog_view;
mod definition;
mod fixtures;
mod gallery_controller;
mod gallery_header;
mod preview;
mod stories;
mod story_host;

use anyhow::{Result, bail};

pub fn run(
    story: Option<String>,
    list: bool,
    snapshot: bool,
    width: Option<u16>,
    height: Option<u16>,
) -> Result<()> {
    if list {
        if story.is_some() || snapshot {
            bail!("--list cannot be combined with a story or --snapshot");
        }
        for story_id in catalog::ids() {
            println!("{story_id}");
        }
        return Ok(());
    }

    let Some(story_id) = story else {
        if snapshot {
            for row in catalog_view::snapshot(width.unwrap_or(100), height.unwrap_or(24))? {
                println!("{row}");
            }
            return Ok(());
        }
        return gallery_controller::run();
    };
    let definition = catalog::by_id(&story_id)?;
    if snapshot {
        let (default_width, default_height) = definition.snapshot_size;
        for row in story_host::snapshot(
            definition,
            width.unwrap_or(default_width),
            height.unwrap_or(default_height),
        )? {
            println!("{row}");
        }
    } else {
        story_host::run(definition)?;
    }
    Ok(())
}
