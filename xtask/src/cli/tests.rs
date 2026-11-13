//! Unit tests of the parent module, against the fixture workspace and the fixture models.

use std::io::{self, Write};
use std::process::ExitCode;

use super::main;

/// A writer whose device is gone.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/arch");

fn fixture(path: &str) -> String {
    format!("{FIXTURES}/{path}")
}

struct Run {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Run {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let code = main(args.iter().copied().chain(None), &mut stdout, &mut stderr);
    Run {
        code,
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

/// The arguments of `arch` against the fixture workspace with the given model.
fn arch<'a>(model: &'a str, workspace: &'a str) -> [&'a str; 6] {
    [
        "xtask",
        "arch",
        "--config",
        model,
        "--manifest-path",
        workspace,
    ]
}

#[test]
fn violations_exit_with_status_one_and_are_printed_to_stdout() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&arch(&model, &workspace));

    assert_eq!(run.code, ExitCode::FAILURE);
    assert!(
        run.stdout.contains("error[arch::layer]: `shop-domain`"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.ends_with("layer(s) checked\n"), "{}", run.stdout);
    // No annotation without the flag depends on `GITHUB_ACTIONS`: tested with a cleared environment
    // in `xtask/tests/integration/arch.rs` (`github_actions_adds_one_annotation_per_violation`).
    assert_eq!(run.stderr, "");
}

#[test]
fn the_github_actions_flag_adds_the_annotations() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));
    let mut args = arch(&model, &workspace).to_vec();
    args.push("--github-actions");

    let run = run(&args);

    assert_eq!(run.code, ExitCode::FAILURE);
    assert_eq!(
        run.stdout.matches("\n::error file=").count(),
        9,
        "{}",
        run.stdout
    );
}

#[test]
fn an_invalid_model_exits_with_status_two_and_says_why_on_stderr() {
    let (model, workspace) = (fixture("invalid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&arch(&model, &workspace));

    assert_eq!(run.code, ExitCode::from(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("architecture check could not run: invalid "),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.ends_with("unknown key `typo` in [[layer]]\n"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_missing_workspace_exits_with_status_two_and_says_why_on_stderr() {
    let (model, workspace) = (
        fixture("valid.toml"),
        fixture("no-such-workspace/Cargo.toml"),
    );

    let run = run(&arch(&model, &workspace));

    assert_eq!(run.code, ExitCode::from(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("architecture check could not run: cannot read the workspace: "),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.ends_with('\n') && !run.stderr.ends_with("\n\n"),
        "{:?}",
        run.stderr
    );
}

#[test]
fn a_missing_model_exits_with_status_two() {
    let workspace = fixture("workspace/Cargo.toml");

    let run = run(&arch("no-such-model.toml", &workspace));

    assert_eq!(run.code, ExitCode::from(2));
    assert!(
        run.stderr
            .starts_with("architecture check could not run: cannot read no-such-model.toml: "),
        "{}",
        run.stderr
    );
}

#[test]
fn no_command_prints_the_usage_on_stderr_and_exits_with_status_two() {
    for args in [
        &["xtask"][..],
        &["xtask", "fmt"],
        &["xtask", "arch", "--config"],
        &["xtask", "arch", "--bogus", "x"],
    ] {
        let run = run(args);

        assert_eq!(run.code, ExitCode::from(2), "{args:?}");
        assert_eq!(run.stdout, "", "{args:?}");
        assert!(
            run.stderr.starts_with("error: ") || run.stderr.contains("Usage: xtask <COMMAND>"),
            "{args:?}: {}",
            run.stderr
        );
    }
}

#[test]
fn help_and_version_are_printed_on_stdout_and_exit_with_status_zero() {
    let help = run(&["xtask", "--help"]);
    let arch_help = run(&["xtask", "arch", "--help"]);
    let version = run(&["xtask", "--version"]);

    for run in [&help, &arch_help, &version] {
        assert_eq!(run.code, ExitCode::SUCCESS);
        assert_eq!(run.stderr, "");
        assert!(
            run.stdout.ends_with('\n') && !run.stdout.ends_with("\n\n"),
            "{:?}",
            run.stdout
        );
    }
    assert!(help.stdout.contains("arch"), "{}", help.stdout);
    assert!(
        arch_help.stdout.contains("--manifest-path"),
        "{}",
        arch_help.stdout
    );
    assert!(
        arch_help.stdout.contains("--github-actions"),
        "{}",
        arch_help.stdout
    );
    assert!(version.stdout.starts_with("xtask "), "{}", version.stdout);
}

#[test]
fn a_report_that_cannot_be_written_is_never_a_pass() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));
    let args = arch(&model, &workspace);

    assert_eq!(main(args, &mut Broken, &mut Vec::new()), ExitCode::from(2));
}

#[test]
fn a_failure_that_cannot_be_written_exits_with_status_two() {
    let (model, workspace) = (fixture("invalid.toml"), fixture("workspace/Cargo.toml"));
    let args = arch(&model, &workspace);

    assert_eq!(main(args, &mut Vec::new(), &mut Broken), ExitCode::from(2));
}

#[test]
fn a_usage_error_that_cannot_be_written_exits_with_status_two() {
    assert_eq!(
        main(["xtask"], &mut Vec::new(), &mut Broken),
        ExitCode::from(2)
    );
}

#[test]
fn help_that_cannot_be_written_exits_with_status_two() {
    assert_eq!(
        main(["xtask", "--help"], &mut Broken, &mut Vec::new()),
        ExitCode::from(2)
    );
}

#[test]
fn the_guard_help_names_its_options_and_the_approval_variable() {
    let help = run(&["xtask", "config-guard", "--help"]);

    assert_eq!(help.code, ExitCode::SUCCESS);
    assert_eq!(help.stderr, "");
    for expected in [
        "--base <REF>",
        "origin/main",
        "--list",
        "--config-change-approved",
        "CONFIG_CHANGE_APPROVED",
    ] {
        assert!(
            help.stdout.contains(expected),
            "{expected}: {}",
            help.stdout
        );
    }
    assert!(run(&["xtask", "--help"]).stdout.contains("config-guard"));
}

#[test]
fn a_base_ref_that_does_not_resolve_exits_with_status_two_and_names_the_ref() {
    let run = run(&["xtask", "config-guard", "--base", "no-such-ref"]);

    assert_eq!(run.code, ExitCode::from(2));
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.stderr,
        "configuration guard could not run: base ref no-such-ref is not available\n"
    );
}

#[test]
fn listing_takes_the_list_of_the_current_directory_and_ignores_the_base_ref() {
    let run = run(&["xtask", "config-guard", "--list", "--base", "no-such-ref"]);

    assert_eq!(run.code, ExitCode::from(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("configuration guard could not run: cannot read protected-config.toml: "),
        "{}",
        run.stderr
    );
}

#[test]
fn a_guard_failure_that_cannot_be_written_exits_with_status_two() {
    let args = ["xtask", "config-guard", "--base", "no-such-ref"];

    assert_eq!(main(args, &mut Vec::new(), &mut Broken), ExitCode::from(2));
}

#[test]
fn a_refused_name_exits_with_status_one_and_says_why_on_stderr() {
    let run = run(&["xtask", "new-context", "Billing", "ledger"]);

    assert_eq!(run.code, ExitCode::FAILURE);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("new-context refused: the group name `Billing` is invalid: "),
        "{}",
        run.stderr
    );
    assert!(run.stderr.ends_with('\n') && !run.stderr.ends_with("\n\n"));
}

#[test]
fn a_reserved_context_exits_with_status_one() {
    let run = run(&["xtask", "new-context", "billing", "shared"]);

    assert_eq!(run.code, ExitCode::FAILURE);
    assert_eq!(
        run.stderr,
        "new-context refused: `shared` is reserved and cannot name a bounded context\n"
    );
}

#[test]
fn both_names_are_required() {
    for args in [
        &["xtask", "new-context"][..],
        &["xtask", "new-context", "billing"],
    ] {
        let run = run(args);

        assert_eq!(run.code, ExitCode::from(2), "{args:?}");
        assert_eq!(run.stdout, "", "{args:?}");
        assert!(
            run.stderr.contains("Usage: xtask new-context"),
            "{}",
            run.stderr
        );
    }
}

#[test]
fn a_refusal_that_cannot_be_written_exits_with_status_two() {
    let args = ["xtask", "new-context", "Billing", "ledger"];

    assert_eq!(main(args, &mut Vec::new(), &mut Broken), ExitCode::from(2));
}

#[test]
fn the_help_names_the_command_and_its_two_names() {
    let help = run(&["xtask", "new-context", "--help"]);

    assert_eq!(help.code, ExitCode::SUCCESS);
    assert!(help.stdout.contains("<GROUP>"), "{}", help.stdout);
    assert!(help.stdout.contains("<CONTEXT>"), "{}", help.stdout);
    assert!(run(&["xtask", "--help"]).stdout.contains("new-context"));
}

#[test]
fn the_approval_is_true_false_or_empty_and_nothing_else() {
    use super::approval;

    assert_eq!(approval("true"), Ok(true));
    assert_eq!(approval("false"), Ok(false));
    assert_eq!(approval(""), Ok(false));
    for other in ["0", "1", "maybe", "TRUE", " true"] {
        assert_eq!(
            approval(other),
            Err(format!("`{other}` is not `true` or `false`"))
        );
    }
}

#[test]
fn the_approval_flag_parses_bare_and_with_a_value() {
    use clap::Parser as _;

    use super::{Cli, Command};

    let approved = |extra: &[&str]| -> Result<Option<bool>, clap::Error> {
        let args = ["xtask", "config-guard"].iter().chain(extra);
        Cli::try_parse_from(args).map(|cli| match cli.command {
            Command::ConfigGuard(guard) => Some(guard.approved),
            Command::Arch(_) | Command::NewContext(_) => None,
        })
    };

    // The default (not approved) depends on `CONFIG_CHANGE_APPROVED`: tested with a cleared
    // environment in `xtask/tests/integration/config_guard.rs`.
    for (flag, expected) in [
        ("--config-change-approved", true),
        ("--config-change-approved=true", true),
        ("--config-change-approved=false", false),
        ("--config-change-approved=", false),
    ] {
        assert_eq!(approved(&[flag]).ok(), Some(Some(expected)), "{flag}");
    }
    assert!(approved(&["--config-change-approved=maybe"]).is_err());
    assert_eq!(
        approved(&["--config-change-approved", "--list"]).ok(),
        Some(Some(true)),
        "the bare flag does not swallow the next option"
    );
}
