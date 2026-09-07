//! Diagnostic commands for individual application layers.
//!
//! They are hidden from the main help and are available under `codediff debug`.

mod align;
mod diff;
mod diff_file;
mod line;
mod list;
mod parity;
mod show;
mod status;
mod ui;

pub use align::print as print_alignment;

use anyhow::Result;

use crate::cli::Debug;

/// Runs a debug command.
pub fn run(command: Debug) -> Result<()> {
    match command {
        Debug::Diff { original, modified } => diff::run(&original, &modified),
        Debug::DiffFile { path, verbose } => diff_file::run(&path, verbose),
        Debug::Align {
            original,
            modified,
            verbose,
        } => align::run(&original, &modified, verbose),
        Debug::Line { file, verbose } => line::run(&file, verbose),
        Debug::Parity {
            original,
            modified,
            layout,
            ignore_trim_whitespace,
        } => parity::run(&original, &modified, layout, ignore_trim_whitespace),
        Debug::Show { spec, raw } => show::run(&spec, raw),
        Debug::List {
            rev,
            staged,
            pathspec,
        } => list::run(list::diff_type(&rev, staged), pathspec),
        Debug::Status { dir, verbose } => status::run(&dir, verbose),
        Debug::Ui {
            story,
            list,
            snapshot,
            width,
            height,
        } => ui::run(story, list, snapshot, width, height),
    }
}
