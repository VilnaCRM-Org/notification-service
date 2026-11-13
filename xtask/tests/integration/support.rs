//! Runs the built `xtask` binary the way the tests need it: with a cleared environment, a working
//! directory outside the repository and the few variables that `cargo` and the coverage tool need.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Variables copied from the test process when it has them: the coverage profile of the child,
/// and what `cargo metadata` needs to be found and to run. Nothing else is forwarded; a test sets
/// any other variable (`GITHUB_ACTIONS`, ...) explicitly.
const FORWARDED: [&str; 6] = [
    "LLVM_PROFILE_FILE",
    "PATH",
    "HOME",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
];

/// The result of one run of the binary.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

/// The command that runs the binary with `arguments` and the variables the test sets.
pub(crate) fn xtask(arguments: &[&str], variables: &[(&str, &str)]) -> Command {
    xtask_in(&empty_directory(), arguments, variables)
}

/// The same command, run in `directory`.
pub(crate) fn xtask_in(
    directory: &Path,
    arguments: &[&str],
    variables: &[(&str, &str)],
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(arguments).env_clear().current_dir(directory);
    forward(&mut command, &FORWARDED);
    command.envs(variables.iter().copied());
    command
}

/// Runs the binary and waits for its exit status.
pub(crate) fn run(arguments: &[&str], variables: &[(&str, &str)]) -> Run {
    collect(&mut xtask(arguments, variables))
}

/// Runs the command and collects its exit code and output.
pub(crate) fn collect(command: &mut Command) -> Run {
    let output = command.output().unwrap();
    Run {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// A file or directory below `tests/fixtures/arch/`, as a string for the command line.
pub(crate) fn fixture(path: &str) -> String {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/arch");
    fixtures.join(path).display().to_string()
}

/// The root of the real workspace, which is the parent of the `xtask` package.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

/// A temporary git repository, removed when it is dropped. It lives in the system temporary
/// directory, never inside the repository under test.
#[derive(Debug)]
pub(crate) struct Repo {
    directory: PathBuf,
}

/// Distinguishes the repositories of the tests of one process.
static REPOSITORIES: AtomicUsize = AtomicUsize::new(0);

impl Repo {
    /// An empty repository with a fixed identity and no commit.
    pub(crate) fn new() -> Self {
        let number = REPOSITORIES.fetch_add(1, Ordering::Relaxed);
        let directory = empty_directory().join(format!("repo-{number}"));
        fs::create_dir_all(&directory).unwrap();
        let repo = Self { directory };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo
    }

    /// Writes a file, creating the directories above it.
    pub(crate) fn write(&self, path: &str, text: &str) {
        let target = self.directory.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }

    /// Deletes a file or a directory with everything in it.
    pub(crate) fn remove(&self, path: &str) {
        let target = self.directory.join(path);
        if fs::symlink_metadata(&target).unwrap().is_dir() {
            fs::remove_dir_all(target).unwrap();
        } else {
            fs::remove_file(target).unwrap();
        }
    }

    /// Replaces whatever is at `path` with a symbolic link to `target`.
    pub(crate) fn link(&self, path: &str, target: &str) {
        let link = self.directory.join(path);
        if fs::symlink_metadata(&link).is_ok() {
            self.remove(path);
        }
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(target, link).unwrap();
    }

    /// Moves a file or a directory inside the repository.
    pub(crate) fn rename(&self, from: &str, to: &str) {
        let target = self.directory.join(to);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::rename(self.directory.join(from), target).unwrap();
    }

    /// Sets or clears the executable bits of a file.
    pub(crate) fn executable(&self, path: &str, executable: bool) {
        let target = self.directory.join(path);
        let mode = if executable { 0o755 } else { 0o644 };
        fs::set_permissions(target, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Commits every change of the working tree.
    pub(crate) fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "--message", message]);
    }

    /// Stages a path without committing it.
    pub(crate) fn stage(&self, path: &str) {
        self.git(&["add", "--", path]);
    }

    /// Runs the binary in the repository.
    pub(crate) fn run(&self, arguments: &[&str], variables: &[(&str, &str)]) -> Run {
        collect(&mut xtask_in(&self.directory, arguments, variables))
    }

    fn git(&self, arguments: &[&str]) {
        let mut command = Command::new("git");
        command
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(arguments)
            .env_clear()
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(&self.directory);
        forward(&mut command, &["PATH", "HOME"]);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap_or_default();
    }
}

/// Copies the named variables from the test process to the command, when the process has them.
pub(crate) fn forward(command: &mut Command, names: &[&str]) {
    for name in names {
        if let Some(value) = from_environment(name) {
            command.env(name, value);
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "the test stack is described by the environment"
)]
fn from_environment(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// A directory under the system temporary directory: never below `target/` and never inside the
/// repository, where a configuration file of the repository could be found from the child.
fn empty_directory() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("xtask-it-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    directory
}
