//! Logging: the JSON subscriber and the panic hook (AD-13).

use shared_domain::BoxError;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

/// Writes the `panic` entry. The hook runs on the panicking thread, so the entry is written
/// inside the request span and carries the request id.
fn log_panic(info: &std::panic::PanicHookInfo<'_>) {
    tracing::error!(panic = %info, "panic");
}

/// Installs the JSON subscriber on `writer` and the panic hook that writes the `panic` entry.
///
/// An entry written inside a span carries the innermost span as `span` and every open span,
/// outermost first, as `spans`: an entry of a nested span inside a request still carries the
/// request span and its request id.
///
/// The level filter takes `tracing_subscriber::EnvFilter` directives.
///
/// # Errors
///
/// Returns an error when the directives are invalid or a subscriber is already installed.
pub fn init<W>(directives: &str, writer: W) -> Result<(), BoxError>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    let filter = EnvFilter::try_new(directives)?;
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(true)
        .with_env_filter(filter)
        .with_writer(writer)
        .try_init()?;
    std::panic::set_hook(Box::new(log_panic));
    Ok(())
}
