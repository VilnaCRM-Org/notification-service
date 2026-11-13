//! The two trees the guard compares: the base ref, read through git, and the working tree.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Failure;

/// The kind of entry a path is, as git records it: the part of the mode that matters to the guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    /// A regular file without the executable bit.
    Plain,
    /// A regular file with the executable bit.
    Executable,
    /// A symbolic link; its content is the link text, never the file it points to.
    Link,
    /// A directory (a path of the working tree only; git lists files).
    Directory,
}

impl Mode {
    /// The mode of a git tree entry (`100644`, `100755`, `120000`, ...).
    fn from_git(mode: &str) -> Self {
        match mode {
            "100755" => Self::Executable,
            "120000" => Self::Link,
            "040000" => Self::Directory,
            _ => Self::Plain,
        }
    }

    /// The mode of a working-tree entry, which is not followed when it is a link.
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        let kind = metadata.file_type();
        if kind.is_symlink() {
            Self::Link
        } else if kind.is_dir() {
            Self::Directory
        } else if metadata.permissions().mode() & 0o111 != 0 {
            Self::Executable
        } else {
            Self::Plain
        }
    }
}

/// What the checks read. Git is reached only through this trait, so unit tests use an in-memory
/// double.
pub(crate) trait BaseTree {
    /// The bytes of a file at the base ref, or `None` when the base ref has no such file.
    fn base_file(&self, path: &str) -> Result<Option<Vec<u8>>, Failure>;
    /// Every file at the base ref.
    fn base_paths(&self) -> Result<Vec<String>, Failure>;
    /// The kind of entry the base ref has at a path, or `None` when it has none.
    fn base_mode(&self, path: &str) -> Result<Option<Mode>, Failure>;
    /// The bytes of a file of the working tree (the link text of a link), or `None` when it is
    /// missing, unreadable, a directory, or below a link.
    fn work_file(&self, path: &str) -> Option<Vec<u8>>;
    /// The kind of entry the working tree has at a path, without following links, or `None` when
    /// it has none or a directory above it is a link.
    fn work_mode(&self, path: &str) -> Option<Mode>;
    /// Every file of the working tree that git tracks or would add.
    fn work_paths(&self) -> Result<Vec<String>, Failure>;
    /// Whether the path is a directory of the working tree.
    fn work_dir(&self, path: &str) -> bool;
}

/// The git implementation: a repository root and the commit that the base ref resolved to.
#[derive(Debug)]
pub(crate) struct Git {
    root: PathBuf,
    commit: String,
}

impl Git {
    /// Resolves `base` to a commit. A ref that cannot be resolved (shallow checkout, missing
    /// remote, not a repository) is an error: a guard that cannot compare never passes.
    pub(crate) fn open(root: &Path, base: &str) -> Result<Self, Failure> {
        let spec = format!("{base}^{{commit}}");
        let id = run(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &spec,
            ],
        )?
        .ok_or_else(|| Failure::BaseUnavailable(base.to_owned()))?;
        Ok(Self {
            root: root.to_owned(),
            commit: String::from_utf8_lossy(&id).trim().to_owned(),
        })
    }
}

impl BaseTree for Git {
    fn base_file(&self, path: &str) -> Result<Option<Vec<u8>>, Failure> {
        let object = format!("{}:./{path}", self.commit);
        run(&self.root, &["cat-file", "blob", &object])
    }

    fn base_paths(&self) -> Result<Vec<String>, Failure> {
        listing(
            &self.root,
            &["ls-tree", "-r", "-z", "--name-only", &self.commit],
        )
    }

    fn base_mode(&self, path: &str) -> Result<Option<Mode>, Failure> {
        let entry = run(&self.root, &["ls-tree", "-z", &self.commit, "--", path])?;
        Ok(entry.as_deref().and_then(entry_mode))
    }

    fn work_file(&self, path: &str) -> Option<Vec<u8>> {
        let target = self.root.join(path);
        match self.work_mode(path)? {
            Mode::Link => fs::read_link(target)
                .ok()
                .map(|link| link.into_os_string().into_encoded_bytes()),
            Mode::Plain | Mode::Executable => fs::read(target).ok(),
            Mode::Directory => None,
        }
    }

    fn work_mode(&self, path: &str) -> Option<Mode> {
        let below_link = Path::new(path)
            .ancestors()
            .skip(1)
            .filter(|ancestor| !ancestor.as_os_str().is_empty())
            .any(|ancestor| {
                fs::symlink_metadata(self.root.join(ancestor))
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
            });
        if below_link {
            return None;
        }
        fs::symlink_metadata(self.root.join(path))
            .ok()
            .map(|metadata| Mode::from_metadata(&metadata))
    }

    fn work_paths(&self) -> Result<Vec<String>, Failure> {
        listing(
            &self.root,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
        )
    }

    fn work_dir(&self, path: &str) -> bool {
        self.root.join(path).is_dir()
    }
}

/// The mode at the start of a line of `git ls-tree` (`<mode> <type> <object>\t<path>`), or `None`
/// when the listing is empty because the path is not in the tree.
fn entry_mode(entry: &[u8]) -> Option<Mode> {
    let text = String::from_utf8_lossy(entry);
    let (mode, _rest) = text.split_once(' ')?;
    Some(Mode::from_git(mode))
}

/// Runs git in `root`: its standard output, or `None` when it exits with a failure status.
fn run(root: &Path, arguments: &[&str]) -> Result<Option<Vec<u8>>, Failure> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()?;
    Ok(output.status.success().then_some(output.stdout))
}

/// Runs a git command that lists files; a listing that failed is an error, never an empty list.
fn listing(root: &Path, arguments: &[&str]) -> Result<Vec<String>, Failure> {
    let output = run(root, arguments)?.ok_or_else(|| Failure::Listing(arguments.join(" ")))?;
    Ok(names(&output))
}

/// The names of a listing whose entries end with a NUL byte.
fn names(listing: &[u8]) -> Vec<String> {
    listing
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect()
}

#[cfg(test)]
mod tests;
