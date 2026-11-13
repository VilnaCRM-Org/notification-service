//! `service migrate` and `service migrate revert`: wait for the database, then apply every pending
//! migration or revert the most recently applied one. Output is plain text.

use std::error::Error;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use shared_infrastructure::persistence::{MIGRATOR, connect_lazy};
use sqlx::migrate::Migrate as _;
use sqlx::{ConnectOptions as _, PgPool};
use tokio::runtime::Runtime;
use tokio::time::Instant;

use crate::cli::{MigrateAction, MigrateArgs};
use crate::command::{self, REJECTED};
use crate::config::{ConfigErrors, MigrateSettings};

/// Pause between two attempts to reach the database.
const ATTEMPT_INTERVAL: Duration = Duration::from_secs(1);

/// Applies the pending migrations (`applied <n> migration(s)`) or, with the action `revert`,
/// reverts the most recent one (`reverted <version>` or `nothing to revert`).
///
/// Returns 0 on success, 1 when the database stays unreachable or a migration fails (the error is
/// printed as `error: <message>`) and 2 for an invalid configuration, which is reported as one
/// line `<VARIABLE>: <reason>` per problem.
pub fn run(args: &MigrateArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    let outcome = match MigrateSettings::from_args(args) {
        Ok(settings) => execute(&settings, args.action, stdout, stderr).map_err(Failure::Run),
        Err(errors) => Err(Failure::Rejected(errors)),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Run(error)) => command::failure(&*error, stderr),
        Err(Failure::Rejected(errors)) => reject(&errors, stderr)
            .map_or_else(|error| command::failure(&error, stderr), |()| rejected()),
    }
}

enum Failure {
    Run(Box<dyn Error>),
    Rejected(ConfigErrors),
}

fn rejected() -> ExitCode {
    ExitCode::from(REJECTED)
}

fn reject(errors: &ConfigErrors, stderr: &mut dyn Write) -> io::Result<()> {
    for problem in errors.problems() {
        writeln!(stderr, "{}: {}", problem.variable, problem.reason)?;
    }
    Ok(())
}

fn execute(
    settings: &MigrateSettings,
    action: Option<MigrateAction>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    // The report goes through `dyn Write` values that cannot cross threads, so the asynchronous
    // parts are small futures driven one at a time and every line is written between them.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let _entered = runtime.enter();
    let pool = connect_lazy(&settings.database)?;
    let wait = Wait {
        attempts: settings.wait_timeout.as_secs().max(1),
        timeout: settings.wait_timeout,
        limit: settings.database.acquire_timeout,
    };
    wait_for_database(&runtime, &pool, wait, stderr)?;
    match action {
        None => {
            let applied = runtime.block_on(apply(&pool))?;
            writeln!(stdout, "applied {applied} migration(s)")?;
        }
        Some(MigrateAction::Revert) => match runtime.block_on(revert(&pool))? {
            Some(version) => writeln!(stdout, "reverted {version}")?,
            None => writeln!(stdout, "nothing to revert")?,
        },
    }
    Ok(())
}

/// Tries to open a connection, one attempt per second, until the wait timeout has elapsed, and
/// reports every failed attempt on `stderr`. The error of the last attempt is returned.
///
/// The timeout is a deadline measured from the first attempt: an attempt is limited to the time
/// left, and no attempt starts once the deadline has passed. A wait shorter than a second still
/// makes one attempt. An attempt is one connection, not a pool acquisition: a pool retries until
/// its own timeout, so the attempts would no longer be a second apart. Each attempt lasts at most
/// `Wait::limit`.
fn wait_for_database(
    runtime: &Runtime,
    pool: &PgPool,
    wait: Wait,
    stderr: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    for number in 1..wait.attempts {
        let limit = wait.limit.min(time_left(started, wait.timeout));
        let Err(error) = attempt(runtime, pool, limit, number, stderr) else {
            return Ok(());
        };
        runtime.block_on(tokio::time::sleep(
            ATTEMPT_INTERVAL.min(time_left(started, wait.timeout)),
        ));
        if time_left(started, wait.timeout).is_zero() {
            return Err(error);
        }
    }
    let limit = wait.limit.min(time_left(started, wait.timeout));
    attempt(runtime, pool, limit, wait.attempts, stderr)
}

fn time_left(started: Instant, timeout: Duration) -> Duration {
    timeout.saturating_sub(started.elapsed())
}

#[derive(Clone, Copy)]
struct Wait {
    /// The most attempts that fit into `timeout`, one per second, and at least one.
    attempts: u64,
    timeout: Duration,
    limit: Duration,
}

fn attempt(
    runtime: &Runtime,
    pool: &PgPool,
    limit: Duration,
    number: u64,
    stderr: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let options = pool.connect_options();
    let connecting = tokio::time::timeout(limit, options.connect());
    let error = match runtime.block_on(connecting) {
        Ok(Ok(_connection)) => return Ok(()),
        Ok(Err(error)) => error,
        Err(_elapsed) => sqlx::Error::Io(io::Error::new(
            io::ErrorKind::TimedOut,
            "the connection attempt timed out",
        )),
    };
    writeln!(stderr, "waiting for database (attempt {number}): {error}")?;
    Err(error.into())
}

/// Applies every pending migration and returns how many were applied.
async fn apply(pool: &PgPool) -> Result<usize, Box<dyn Error>> {
    let mut connection = pool.acquire().await?;
    connection
        .ensure_migrations_table(&MIGRATOR.table_name)
        .await?;
    let before = connection
        .list_applied_migrations(&MIGRATOR.table_name)
        .await?
        .len();
    MIGRATOR.run(&mut *connection).await?;
    let after = connection
        .list_applied_migrations(&MIGRATOR.table_name)
        .await?
        .len();
    Ok(after.saturating_sub(before))
}

/// Reverts the most recently applied migration and returns its version, or `None` when no
/// migration is applied.
async fn revert(pool: &PgPool) -> Result<Option<i64>, Box<dyn Error>> {
    let mut connection = pool.acquire().await?;
    connection
        .ensure_migrations_table(&MIGRATOR.table_name)
        .await?;
    let applied = connection
        .list_applied_migrations(&MIGRATOR.table_name)
        .await?;
    let Some(latest) = applied.last() else {
        return Ok(None);
    };
    // `undo` reverts every applied version above its target; the latest is the highest applied.
    MIGRATOR
        .undo(&mut *connection, latest.version.saturating_sub(1))
        .await?;
    Ok(Some(latest.version))
}
