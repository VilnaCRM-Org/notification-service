//! PostgreSQL pool factory and the migrations embedded in the binary.

use std::str::FromStr as _;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

/// Every migration of the `migrations/` directory at the workspace root, embedded at compile time.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../../migrations");

/// Shortest `statement_timeout` the pool sends: PostgreSQL reads 0 as "no timeout", so anything
/// below one millisecond is raised to this value.
pub const MIN_STATEMENT_TIMEOUT: Duration = Duration::from_millis(1);

/// Longest `statement_timeout` the pool sends: 24 days. PostgreSQL stores the value as a 32-bit
/// count of milliseconds (about 24.8 days) and refuses a connection that asks for more.
pub const MAX_STATEMENT_TIMEOUT: Duration = Duration::from_hours(24 * 24);

/// Settings of the PostgreSQL pool.
///
/// `url` carries the password: never format a value of this type with `{:?}` and never log it.
#[derive(Debug, Clone)]
pub struct PostgresSettings {
    /// `postgres://` or `postgresql://` URL of the database.
    pub url: String,
    /// Upper bound of open connections.
    pub max_connections: u32,
    /// How long a caller waits for a free connection.
    pub acquire_timeout: Duration,
    /// `statement_timeout` set on every connection; [`connect_lazy`] keeps it between
    /// [`MIN_STATEMENT_TIMEOUT`] and [`MAX_STATEMENT_TIMEOUT`].
    pub statement_timeout: Duration,
}

/// Builds the pool without opening a connection, so the service starts while the database is down.
///
/// # Errors
///
/// Returns the parse error when `settings.url` is not a valid PostgreSQL URL.
pub fn connect_lazy(settings: &PostgresSettings) -> Result<PgPool, sqlx::Error> {
    let timeout = statement_timeout_millis(settings.statement_timeout);
    let options =
        PgConnectOptions::from_str(&settings.url)?.options([("statement_timeout", timeout)]);
    Ok(PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout)
        .connect_lazy_with(options))
}

/// The `statement_timeout` value in whole milliseconds, never 0 and never above the maximum.
fn statement_timeout_millis(timeout: Duration) -> String {
    timeout
        .clamp(MIN_STATEMENT_TIMEOUT, MAX_STATEMENT_TIMEOUT)
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests;
