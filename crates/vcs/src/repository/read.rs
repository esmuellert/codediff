//! Reading one file: its content on one side, or raw bytes at a revision.

use file_types::{DiffVersion, File, FileContent, RepoPath};

use crate::git;

use super::Repository;

impl Repository {
    /// Reads one side of a file, including the old path of a rename.
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

    /// Reads raw bytes at a revision for verification.
    ///
    /// Returns `None` when the path is absent.
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
