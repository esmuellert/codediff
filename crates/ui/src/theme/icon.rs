//! The glyph and colour to draw beside a file name.
//!
//! The tables are nvim-web-devicons' own, converted to Rust — see
//! `ATTRIBUTION.md`. A glyph only renders if the terminal font has the Nerd
//! Font range; nothing here checks that.

use ratatui::style::Color;

mod table;

pub use table::{EXTENSIONS, FILENAMES};

/// A glyph and the colour to draw it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub glyph: char,
    pub color: Color,
}

impl Icon {
    /// `rgb` is `0xRRGGBB` — the `#rrggbb` upstream writes.
    pub const fn new(glyph: char, rgb: u32) -> Self {
        Self {
            glyph,
            color: Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
        }
    }
}

/// A file neither table knows.
pub const FILE: Icon = Icon::new('\u{f15b}', 0x6d8086);

/// A directory. Upstream ships no folder icons; these are nvim-tree's, in the
/// colour its `NvimTreeFolderIcon` group uses.
pub const FOLDER_CLOSED: Icon = Icon::new('\u{e5ff}', 0x8094b4);
pub const FOLDER_OPEN: Icon = Icon::new('\u{e5fe}', 0x8094b4);

pub const fn folder(open: bool) -> Icon {
    if open { FOLDER_OPEN } else { FOLDER_CLOSED }
}

/// The icon for a path, or `None` if neither table names it.
///
/// The whole file name is tried first, then each extension from the longest
/// down: `Button.spec.tsx` matches `spec.tsx` before `tsx`. Any directory part
/// is dropped, and ASCII case is ignored.
pub fn lookup(path: &str) -> Option<Icon> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if let Some(icon) = find(FILENAMES, name) {
        return Some(icon);
    }

    let mut rest = name;
    while let Some((_, extension)) = rest.split_once('.') {
        if let Some(icon) = find(EXTENSIONS, extension) {
            return Some(icon);
        }
        rest = extension;
    }
    None
}

/// The same, falling back to [`FILE`].
pub fn file(path: &str) -> Icon {
    lookup(path).unwrap_or(FILE)
}

/// Searches a table whose keys are lowercase and byte-ordered.
fn find(table: &[(&str, Icon)], key: &str) -> Option<Icon> {
    let index = table
        .binary_search_by(|(entry, _)| entry.bytes().cmp(key.bytes().map(lower)))
        .ok()?;
    Some(table[index].1)
}

const fn lower(byte: u8) -> u8 {
    byte.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST: Icon = Icon::new('\u{e68b}', 0xdea584);

    /// What [`find`] assumes, and cannot check for itself.
    #[test]
    fn both_tables_are_lowercase_and_byte_ordered() {
        for table in [FILENAMES, EXTENSIONS] {
            for pair in table.windows(2) {
                assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
            }
            for (key, _) in table {
                assert_eq!(*key, key.to_lowercase(), "{key}");
            }
        }
    }

    #[test]
    fn nothing_was_trimmed_on_the_way_across() {
        assert_eq!(FILENAMES.len(), 217);
        // Upstream has 493, of which `R` folds onto `r`.
        assert_eq!(EXTENSIONS.len(), 492);
    }

    #[test]
    fn an_extension_carries_the_colour_upstream_gives_it() {
        assert_eq!(lookup("main.rs"), Some(RUST));
        assert_eq!(RUST.color, Color::Rgb(0xde, 0xa5, 0x84));
    }

    #[test]
    fn a_whole_name_beats_the_extension_it_ends_in() {
        // `.yml` would match too, and says something less specific.
        assert_ne!(lookup(".gitlab-ci.yml"), lookup("deploy.yml"));
        assert_eq!(lookup("CMakeLists.txt"), lookup("cmakelists.txt"));
        assert_ne!(lookup("CMakeLists.txt"), lookup("readme.txt"));
    }

    #[test]
    fn the_longest_extension_wins() {
        assert_ne!(lookup("Button.spec.tsx"), lookup("Button.tsx"));
        assert_eq!(lookup("Button.spec.tsx"), lookup("a.b.c.spec.tsx"));
    }

    #[test]
    fn a_directory_part_is_dropped() {
        assert_eq!(lookup("crates/ui/src/main.rs"), Some(RUST));
        assert_eq!(lookup(r"crates\ui\main.rs"), Some(RUST));
        // A dot in a directory is not an extension of the file.
        assert_eq!(lookup("v1.2/README"), lookup("README"));
    }

    #[test]
    fn case_is_ignored_on_both_sides() {
        assert_eq!(lookup("MAIN.RS"), Some(RUST));
        assert_eq!(lookup("Makefile"), lookup("makefile"));
        assert_eq!(lookup(".BASHRC"), lookup(".bashrc"));
    }

    #[test]
    fn a_non_ascii_key_is_still_found() {
        assert!(lookup("burn.🔥").is_some());
    }

    #[test]
    fn a_name_nothing_matches_falls_back() {
        assert_eq!(lookup("notes.qqzz"), None);
        assert_eq!(lookup(""), None);
        assert_eq!(lookup("."), None);
        assert_eq!(file("notes.qqzz"), FILE);
        assert_eq!(file("main.rs"), RUST);
    }

    #[test]
    fn a_folder_looks_different_open_and_closed() {
        assert_ne!(folder(true).glyph, folder(false).glyph);
        assert_eq!(folder(true), FOLDER_OPEN);
        assert_eq!(folder(false), FOLDER_CLOSED);
    }
}
