//! `service healthcheck`: requests `/ping` on the address the service listens on (the loopback
//! interface when it listens on every interface), with no dependency access.

use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

use crate::cli::HealthcheckArgs;

/// Connect, write and read timeout of the probe.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Port probed when neither `--port` nor `APP_PORT` is given.
const DEFAULT_PORT: u16 = 8081;

/// Sends `GET /ping` over a plain TCP connection and reports whether the status line is 200.
#[expect(
    clippy::disallowed_methods,
    reason = "the liveness probe owns this loopback connection"
)]
fn probe(address: SocketAddr) -> std::io::Result<bool> {
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    stream.write_all(b"GET /ping HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response.starts_with("HTTP/1.1 200"))
}

/// The address to probe for the configured `APP_HOST`: the loopback address when the variable is
/// absent or names an unspecified address (`0.0.0.0` or `::` accept connections on the loopback
/// interface of their family), the address itself otherwise. `None` when it is not an IP address.
fn target_host(host: Option<&str>) -> Option<IpAddr> {
    let Some(text) = host else {
        return Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    };
    let address = text.parse::<IpAddr>().ok()?;
    if !address.is_unspecified() {
        return Some(address);
    }
    Some(if address.is_ipv4() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        IpAddr::V6(Ipv6Addr::LOCALHOST)
    })
}

/// Requests `/ping` on the address the service listens on and the port.
///
/// The address is `--host` / `APP_HOST` (loopback by default). The exit status is 0 when the
/// answer is 200 and 1 otherwise, including an unparsable host or port, a refused connection and a
/// timeout. Prints nothing.
#[must_use]
pub fn run(args: &HealthcheckArgs) -> ExitCode {
    let port = args
        .port
        .as_deref()
        .map_or(Some(DEFAULT_PORT), |text| text.parse().ok());
    let host = target_host(args.host.as_deref());
    match port
        .zip(host)
        .map(|(port, host)| probe(SocketAddr::new(host, port)))
    {
        Some(Ok(true)) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests;
