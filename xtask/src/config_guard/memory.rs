//! An in-memory `BaseTree` for unit tests: the base ref and the working tree are maps.

use std::collections::{BTreeMap, BTreeSet};

use super::Failure;
use super::base::{BaseTree, Mode};

/// Files of the base ref, files of the working tree and the directories of the working tree.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    base: BTreeMap<String, Vec<u8>>,
    work: BTreeMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
    base_modes: BTreeMap<String, Mode>,
    work_modes: BTreeMap<String, Mode>,
}

impl Memory {
    /// A file with the same content on the base ref and in the working tree.
    pub(crate) fn same(self, path: &str, text: &str) -> Self {
        self.on_base(path, text).in_work(path, text)
    }

    /// A file of the base ref only (deleted in the working tree).
    pub(crate) fn on_base(mut self, path: &str, text: &str) -> Self {
        self.base.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// A file of the working tree only (new, or the changed content of a base file).
    pub(crate) fn in_work(mut self, path: &str, text: &str) -> Self {
        self.work.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// The kind of entry of a path at the base ref, when it is not a plain file.
    pub(crate) fn base_as(mut self, path: &str, mode: Mode) -> Self {
        self.base_modes.insert(path.to_owned(), mode);
        self
    }

    /// The kind of entry of a path in the working tree, when it is not a plain file.
    pub(crate) fn work_as(mut self, path: &str, mode: Mode) -> Self {
        self.work_modes.insert(path.to_owned(), mode);
        self
    }

    /// A directory of the working tree.
    pub(crate) fn directory(mut self, path: &str) -> Self {
        self.directories.insert(path.to_owned());
        self
    }
}

impl BaseTree for Memory {
    fn base_file(&self, path: &str) -> Result<Option<Vec<u8>>, Failure> {
        Ok(self.base.get(path).cloned())
    }

    fn base_paths(&self) -> Result<Vec<String>, Failure> {
        Ok(self.base.keys().cloned().collect())
    }

    fn base_mode(&self, path: &str) -> Result<Option<Mode>, Failure> {
        Ok(self
            .base
            .contains_key(path)
            .then(|| self.base_modes.get(path).copied().unwrap_or(Mode::Plain)))
    }

    fn work_file(&self, path: &str) -> Option<Vec<u8>> {
        self.work.get(path).cloned()
    }

    fn work_mode(&self, path: &str) -> Option<Mode> {
        if let Some(mode) = self.work_modes.get(path) {
            return Some(*mode);
        }
        if self.work.contains_key(path) {
            Some(Mode::Plain)
        } else {
            self.directories.contains(path).then_some(Mode::Directory)
        }
    }

    fn work_paths(&self) -> Result<Vec<String>, Failure> {
        Ok(self.work.keys().cloned().collect())
    }

    fn work_dir(&self, path: &str) -> bool {
        self.directories.contains(path)
    }
}
