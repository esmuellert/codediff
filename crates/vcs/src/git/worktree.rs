//! Reading the working tree — the checkout on disk.
//!
//! Not a git command: it is `std::fs`. It lives here because the working tree
//! is one of the two sides of the default comparison, and belongs behind the
//! same interface as the side that does come from the object store.

use crate::error::{Error, Result};
use file_types::RepoPath;

/// A file's current content. `None` when it is not on disk — a deletion, or a
/// path that only exists in the revision being compared against.
///
/// Takes no root: a [`RepoPath`] already carries its absolute form, which is
/// the reason it carries one. Passing the two separately is how they come to
/// disagree.
pub fn read(path: &RepoPath) -> Result<Option<Vec<u8>>> {
    // A symlink's content, to git, is where it points — a short line of text,
    // stored as a blob. Reading through it would compare the *target* file
    // against that line, which makes an unchanged link look like a whole file
    // rewritten. `symlink_metadata` is what does not follow.
    match std::fs::symlink_metadata(path.as_path()) {
        Ok(meta) if meta.is_symlink() => {
            return match std::fs::read_link(path.as_path()) {
                Ok(target) => Ok(Some(target.into_os_string().into_encoded_bytes())),
                Err(source) => Err(Error::Io {
                    path: path.as_path().to_path_buf(),
                    source,
                }),
            };
        }
        // A directory here is a submodule, whose content is a commit id rather
        // than bytes. Answered as absent, so the reviewer says the file cannot
        // be shown instead of failing to read a directory.
        Ok(meta) if meta.is_dir() => return Ok(None),
        _ => {}
    }
    match std::fs::read(path.as_path()) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.as_path().to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symlink_reads_as_where_it_points() {
        let dir = std::env::temp_dir().join("codediff-symlink-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory");
        std::fs::write(
            dir.join("real.txt"),
            "many
lines
of
text
",
        )
        .expect("a file");
        std::os::unix::fs::symlink("real.txt", dir.join("link.txt")).expect("a link");

        let link = RepoPath::new("link.txt", &dir);
        assert_eq!(
            read(&link).expect("readable"),
            Some(b"real.txt".to_vec()),
            "the target, not the file it points at"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_submodule_directory_is_absent_rather_than_an_error() {
        let dir = std::env::temp_dir().join("codediff-submodule-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).expect("a directory");
        assert_eq!(read(&RepoPath::new("sub", &dir)).expect("no error"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
