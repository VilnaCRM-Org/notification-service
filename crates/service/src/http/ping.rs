//! Liveness: `GET /ping` answers `pong` and touches no dependency (AD-12).

/// Answers `pong` as `text/plain; charset=utf-8`.
///
/// The handler takes no state, so it keeps answering while every dependency is down.
pub async fn ping() -> &'static str {
    "pong"
}

#[cfg(test)]
mod tests;
