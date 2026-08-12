//! Reading one file: its content on one side, or raw bytes at a revision.

use file_types::{DiffVersion, File, FileContent, RepoPath};

use crate::git;

use super::Repository;

impl Repository {
    /// One side of one file.
    ///
    /// Takes the whole [`File`] rather than a path so that a move reads its
    /// old path without the caller having to know that rule.
    pub fn get_file_content(
        &mut self,
        file: &File,
        version: DiffVersion,
    ) -> crate::Result<FileContent> {
        tracing::info!(path = %file.path(), ?version, "reading file");
        if self.blobs.is_none() {
            self.blobs = Some(crate::git::cat_file::Batch::open(&self.repo)?);
        }
        let blobs = self.blobs.as_mut().expect("just opened");
        git::read(&self.repo, blobs, file, version)
    }

    /// One path as it was at one revision, exactly.
    ///
    /// Not part of reviewing anything — [`get_file_content`](Self::get_file_content) is what
    /// a review uses. This is for checking that what we read is byte for byte
    /// what the backend holds.
    ///
    /// `None` when nothing is there at that revision.
    pub fn get_raw_content(
        &mut self,
        rev: &str,
        path: &RepoPath,
    ) -> crate::Result<Option<Vec<u8>>> {
        if self.blobs.is_none() {
            self.blobs = Some(crate::git::cat_file::Batch::open(&self.repo)?);
        }
        self.blobs.as_mut().expect("just opened").read(rev, path)
    }
}
