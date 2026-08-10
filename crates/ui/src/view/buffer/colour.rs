//! Asking the syntax worker for colour and reading the answer back.
//!
//! Shared by all buffer types showing a file. A diff asks about two
//! versions, a single file about one.

use std::sync::Arc;

use channel::Worker;
use file_types::{DiffVersion, File};
use pipeline::file;

use syntax::{Colours, Spans, Store, Syntax, SyntaxRequest, Version, path_of};

/// Asks for both versions of a paired file, up to `last`.
pub fn request_diff(
    read: &file::Diff,
    syntax: &mut Syntax,
    store: &mut Store,
    version: Version,
    last: u32,
) {
    for side in [DiffVersion::Original, DiffVersion::Modified] {
        request(
            syntax,
            store,
            &read.file,
            side,
            read.alignment.text(side),
            version,
            last,
        );
    }
}

/// Asks for the one version a lone file has, up to `last`.
pub fn request_single_file(
    read: &file::SingleFile,
    syntax: &mut Syntax,
    store: &mut Store,
    version: Version,
    last: u32,
) {
    request(
        syntax,
        store,
        &read.file,
        read.side(),
        Arc::clone(&read.lines),
        version,
        last,
    );
}

/// How both versions of a paired file are coloured, for a frame.
pub fn spans_diff<'a>(read: &file::Diff, store: &'a Store) -> Spans<'a> {
    Spans::Both {
        original: colours(store, &read.file, DiffVersion::Original),
        modified: colours(store, &read.file, DiffVersion::Modified),
    }
}

/// How the one version of a lone file is coloured, for a frame.
pub fn spans_single_file<'a>(read: &file::SingleFile, store: &'a Store) -> Spans<'a> {
    match colours(store, &read.file, read.side()) {
        Some(read) => Spans::One(read),
        None => Spans::Off,
    }
}

/// Asks for one version of one file, up to `upto`.
///
/// Sends nothing when the store already holds enough (the ordinary case
/// after the first screen); when a request for that version is still
/// outstanding, since what was wanted meanwhile is asked for again on the
/// next frame from a starting point that is current by then; or when the
/// file does not exist on the side asked about, which has no text and so
/// no language.
fn request(
    syntax: &mut Syntax,
    store: &mut Store,
    file: &File,
    side: DiffVersion,
    text: Arc<Vec<String>>,
    version: Version,
    upto: u32,
) {
    let (Some(key), Some(path)) = (file.name(side), path_of(file, side)) else {
        return;
    };
    let lines = text.len() as u32;
    if lines == 0 || syntax.busy(&key) {
        return;
    }
    let last = upto.min(lines - 1);
    store.ensure_cache(&key, version);
    let have = store.get_lines_coloured(&key);
    if have > last {
        return;
    }
    syntax.send(SyntaxRequest {
        key,
        path,
        version,
        text,
        have,
        last,
    });
}

/// What has been coloured of one version, if anything.
///
/// The name is asked of the file each time rather than held, because it is
/// derived — storing it beside the file is how a copy comes to disagree
/// with what it was copied from.
fn colours<'a>(store: &'a Store, file: &File, side: DiffVersion) -> Option<&'a Colours> {
    file.name(side).and_then(|key| store.get_colours(&key))
}

#[cfg(test)]
mod tests {
    use align::Alignment;
    use file_types::{File, Oid, RepoPath, Rev, Revs};

    use super::*;
    use syntax::{Store, Syntax, Version};

    fn at(path: &str) -> RepoPath {
        RepoPath::new(path, std::path::Path::new("/repo"))
    }

    /// A file against itself. No engine is run — `ui` may not name one — and
    /// none is needed: what these check is which entries a request makes, not
    /// what the pairing says.
    fn alignment(lines: &[&str]) -> Alignment {
        Alignment::new(
            diff_types::LinesDiff {
                changes: Vec::new(),
                moves: Vec::new(),
                hit_timeout: false,
            },
            lines,
            lines,
        )
    }

    /// One diff of one path, read against `HEAD`, with the after side named.
    fn diff(after: Rev) -> pipeline::file::Diff {
        let revs = Revs::new(Rev::Commit(Oid::new("b87b24c")), after);
        pipeline::file::Diff {
            file: File::unchanged_path(at("src/main.rs"), revs),
            alignment: alignment(&["fn main() {}"]),
        }
    }

    #[test]
    fn the_staged_and_the_working_copy_of_one_path_do_not_share_a_cache_entry() {
        // The old key said which column a version was drawn in, so both of
        // these were one name over two different sets of bytes.
        let (emitter, _rx) = channel::Emitter::local();
        let mut syntax = Syntax::start(emitter);
        let mut store = Store::new();

        for after in [Rev::Worktree, Rev::Index] {
            request_diff(&diff(after), &mut syntax, &mut store, Version(1), 0);
        }

        assert_eq!(
            store.cached_count(),
            3,
            "one entry for the shared before side, and one for each after side"
        );
    }

    #[test]
    fn two_files_read_against_one_commit_share_that_side() {
        // The other half, and free: a commit is named by its id, so the before
        // side of every file in a review that happens to be the same blob is
        // the same entry.
        let (emitter, _rx) = channel::Emitter::local();
        let mut syntax = Syntax::start(emitter);
        let mut store = Store::new();
        request_diff(&diff(Rev::Worktree), &mut syntax, &mut store, Version(1), 0);
        let before = store.cached_count();
        request_diff(&diff(Rev::Worktree), &mut syntax, &mut store, Version(1), 0);
        assert_eq!(
            store.cached_count(),
            before,
            "asking twice made no new entry"
        );
    }
}
