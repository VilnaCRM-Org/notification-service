//! Unit tests of the parent module. The git implementation is exercised against real
//! repositories by `tests/integration/config_guard.rs`; here are the parts that need none.

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::PathBuf;

use super::{BaseTree as _, Failure, Git, Mode, entry_mode, names, run};

/// A directory that is not inside a git repository.
fn outside_a_repository() -> PathBuf {
    std::env::temp_dir()
}

#[test]
fn a_listing_is_split_at_the_nul_bytes_and_empty_names_are_dropped() {
    assert_eq!(names(b"a/b\0c d\0\0e\xc3\xa9\0"), ["a/b", "c d", "e\u{e9}"]);
    assert_eq!(names(b"no-terminator"), ["no-terminator"]);
    assert_eq!(names(b""), Vec::<String>::new());
}

#[test]
fn a_failing_git_command_has_no_output_and_a_missing_directory_is_an_error() {
    let root = outside_a_repository();

    assert_eq!(run(&root, &["rev-parse", "--git-dir"]).unwrap(), None);
    assert!(matches!(
        run(&root.join("no-such-directory"), &["--version"]),
        Err(Failure::Git(_))
    ));
}

#[test]
fn a_base_ref_that_cannot_be_resolved_is_reported_with_its_name() {
    let error = Git::open(&outside_a_repository(), "origin/main").unwrap_err();

    assert!(matches!(&error, Failure::BaseUnavailable(base) if base == "origin/main"));
    assert_eq!(error.to_string(), "base ref origin/main is not available");
}

#[test]
fn a_listing_that_fails_is_an_error_and_not_an_empty_list() {
    let git = Git {
        root: outside_a_repository(),
        commit: "0".repeat(40),
    };

    assert_eq!(git.base_file("clippy.toml").unwrap(), None);
    assert_eq!(git.base_mode("clippy.toml").unwrap(), None);
    let base = git.base_paths().unwrap_err();
    assert!(matches!(&base, Failure::Listing(command) if command.starts_with("ls-tree ")));
    let work = git.work_paths().unwrap_err();
    assert!(matches!(&work, Failure::Listing(command) if command.starts_with("ls-files ")));
    assert_eq!(
        work.to_string(),
        format!(
            "`git {}` failed",
            "ls-files -z --cached --others --exclude-standard"
        )
    );
}

#[test]
fn the_working_tree_is_read_from_the_file_system() {
    let git = Git {
        root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        commit: String::new(),
    };

    assert!(git.work_file("Cargo.toml").is_some());
    assert_eq!(git.work_file("no-such-file"), None);
    assert_eq!(git.work_file("src"), None);
    assert!(git.work_dir("src"));
    assert!(!git.work_dir("Cargo.toml"));
    assert!(!git.work_dir("no-such-directory"));
}

#[test]
fn a_git_mode_is_a_kind_of_entry() {
    assert_eq!(Mode::from_git("100644"), Mode::Plain);
    assert_eq!(Mode::from_git("100755"), Mode::Executable);
    assert_eq!(Mode::from_git("120000"), Mode::Link);
    assert_eq!(Mode::from_git("040000"), Mode::Directory);
    assert_eq!(Mode::from_git("160000"), Mode::Plain);
}

#[test]
fn the_working_tree_is_read_without_following_links() {
    let root = std::env::temp_dir().join(format!("xtask-base-{}", std::process::id()));
    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/file"), "bytes").unwrap();
    fs::write(root.join("script"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(root.join("script"), fs::Permissions::from_mode(0o755)).unwrap();
    symlink("real", root.join("link")).unwrap();
    symlink("real/file", root.join("file-link")).unwrap();
    let git = Git {
        root: root.clone(),
        commit: String::new(),
    };

    assert_eq!(git.work_mode("real"), Some(Mode::Directory));
    assert_eq!(git.work_mode("real/file"), Some(Mode::Plain));
    assert_eq!(git.work_mode("script"), Some(Mode::Executable));
    assert_eq!(git.work_mode("link"), Some(Mode::Link));
    assert_eq!(git.work_mode("file-link"), Some(Mode::Link));
    assert_eq!(git.work_mode("link/file"), None);
    assert_eq!(git.work_mode("missing"), None);
    assert_eq!(git.work_file("real/file"), Some(b"bytes".to_vec()));
    assert_eq!(git.work_file("file-link"), Some(b"real/file".to_vec()));
    assert_eq!(git.work_file("link"), Some(b"real".to_vec()));
    assert_eq!(git.work_file("link/file"), None);
    assert_eq!(git.work_file("real"), None);
    assert_eq!(git.work_file("missing"), None);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_mode_is_read_from_the_start_of_a_tree_entry() {
    assert_eq!(
        entry_mode(b"100755 blob 0123abc\tdir/script\0"),
        Some(Mode::Executable)
    );
    assert_eq!(entry_mode(b"120000 blob 0123abc\tlink\0"), Some(Mode::Link));
    assert_eq!(entry_mode(b""), None);
}
