//! `codediff debug ui` — deterministic production-component stories.

mod browse;
mod browser;
mod catalog;
mod catalog_rows;
mod chrome;
mod component;
mod definition;
mod fixtures;
mod session;
mod stories;

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
        for name in catalog::names() {
            println!("{name}");
        }
        return Ok(());
    }

    let Some(name) = story else {
        if snapshot {
            for row in browser::snapshot(width.unwrap_or(100), height.unwrap_or(24))? {
                println!("{row}");
            }
            return Ok(());
        }
        return browse::run();
    };
    let definition = catalog::named(&name)?;
    if snapshot {
        let (default_width, default_height) = definition.default_size;
        for row in session::snapshot(
            definition,
            width.unwrap_or(default_width),
            height.unwrap_or(default_height),
        )? {
            println!("{row}");
        }
    } else {
        session::run(definition)?;
    }
    Ok(())
}
