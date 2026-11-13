//! Spawns the service binary with a cleared environment in a scratch directory outside the repository.

use std::ffi::OsString;
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::Value;

/// Every variable of a valid configuration that has no default: the database and the region.
pub(crate) const VALID_ENVIRONMENT: [(&str, &str); 2] = [
    (
        "DATABASE_URL",
        "postgres://app:local-dev-only@127.0.0.1:5432/app",
    ),
    ("AWS_REGION", "us-east-1"),
];

/// URL that nothing listens on: the "unreachable database" case. No container is stopped.
pub(crate) const DEAD_DATABASE_URL: &str = "postgres://app:unused@127.0.0.1:1/app";

static SEQUENCE: AtomicU32 = AtomicU32::new(0);

/// An empty directory under the system temporary directory, removed when the value is dropped.
///
/// It is never below `target/` or the repository: `dotenvy` also searches the parent directories
/// and would find the committed `.env`.
#[derive(Debug)]
pub(crate) struct Scratch {
    path: PathBuf,
}

impl Scratch {
    pub(crate) fn new() -> Self {
        let number = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("service-it-{}-{number}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a file into the directory.
    pub(crate) fn write(&self, name: &str, contents: &str) {
        fs::write(self.path.join(name), contents).unwrap();
    }

    /// The binary with a cleared environment plus `variables`, and `LLVM_PROFILE_FILE` when the
    /// test process has it, so that coverage of the child is recorded.
    pub(crate) fn service(&self, arguments: &[&str], variables: &[(&str, &str)]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_service"));
        command.args(arguments).env_clear().current_dir(&self.path);
        if let Some(profile) = from_environment("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command.envs(variables.iter().copied());
        command
    }

    /// Runs the binary to its end and collects its output.
    pub(crate) fn run(&self, arguments: &[&str], variables: &[(&str, &str)]) -> Output {
        self.service(arguments, variables).output().unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // A directory that cannot be removed is left to the operating system's temporary cleanup.
        let _removed = fs::remove_dir_all(&self.path);
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "the test stack is described by the environment"
)]
fn from_environment(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// Standard output as text.
pub(crate) fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

/// Standard error as text.
pub(crate) fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// The URL of the database a `#[sqlx::test]` gave the test: its name from the server, the rest
/// from `DATABASE_URL`.
pub(crate) async fn database_url(pool: &sqlx::PgPool) -> String {
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = from_environment("DATABASE_URL")
        .expect("DATABASE_URL is not set; run the tests through `make`")
        .into_string()
        .unwrap();
    let (server, _database) = base.rsplit_once('/').unwrap();
    format!("{server}/{name}")
}

/// A listener on a free loopback port that accepts connections (the kernel does) and never
/// answers: the peer connects and then waits.
#[expect(
    clippy::disallowed_methods,
    reason = "integration tests open the sockets their subject talks to"
)]
pub(crate) fn silent_listener() -> std::net::TcpListener {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap()
}

/// A variable of the test stack (`AWS_ENDPOINT_URL_SQS` and the like), taken from the environment
/// of the test process.
pub(crate) fn stack_variable(name: &str) -> String {
    from_environment(name)
        .unwrap_or_else(|| panic!("{name} is not set; run the tests through `make`"))
        .into_string()
        .unwrap()
}

/// The variables that let `serve` start against the compose services: the AWS ones of the test
/// stack and the given database URL, with long budgets so that a request that must succeed does
/// not depend on how fast a loaded machine opens its first connection (request > probe >=
/// acquire and operation, as the configuration requires).
pub(crate) fn stack_with(database_url: &str) -> Vec<(&'static str, String)> {
    vec![
        ("DATABASE_URL", database_url.to_owned()),
        ("AWS_REGION", stack_variable("AWS_REGION")),
        (
            "AWS_ENDPOINT_URL_SQS",
            stack_variable("AWS_ENDPOINT_URL_SQS"),
        ),
        ("AWS_ACCESS_KEY_ID", stack_variable("AWS_ACCESS_KEY_ID")),
        (
            "AWS_SECRET_ACCESS_KEY",
            stack_variable("AWS_SECRET_ACCESS_KEY"),
        ),
        ("DATABASE_ACQUIRE_TIMEOUT", "30s".to_owned()),
        ("SQS_CONNECT_TIMEOUT", "10s".to_owned()),
        ("SQS_OPERATION_TIMEOUT", "30s".to_owned()),
        ("HEALTH_PROBE_TIMEOUT", "40s".to_owned()),
        ("HTTP_REQUEST_TIMEOUT", "60s".to_owned()),
    ]
}

/// An answer of the service to one HTTP request.
#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    /// The status line and the headers, header names in lower case.
    pub(crate) head: String,
    pub(crate) body: String,
}

impl Answer {
    fn parse(raw: &str) -> Self {
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        Self {
            status,
            head: head.to_lowercase(),
            body: body.to_owned(),
        }
    }
}

/// A running `service serve` process with its standard output read as JSON lines.
///
/// A server that is dropped while it still runs is killed, so that no test leaks a process.
#[derive(Debug)]
pub(crate) struct Server {
    child: Child,
    /// Lines of standard output, read by a thread so that a wait for a line can time out.
    stdout: Receiver<String>,
    stderr: ChildStderr,
    /// The bound port.
    pub(crate) port: u16,
    /// Every log line read so far.
    pub(crate) lines: Vec<Value>,
}

/// How long a test waits for the next line of a service before it fails.
const LINE_TIMEOUT: Duration = Duration::from_secs(20);

/// Reads the lines of `stdout` on a thread; the channel closes when the process closes it.
fn lines_of(stdout: BufReader<std::process::ChildStdout>) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in stdout.lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// How a `serve` process ended.
#[derive(Debug)]
pub(crate) struct Finished {
    pub(crate) code: Option<i32>,
    /// Every log line of the process, in order.
    pub(crate) lines: Vec<Value>,
    pub(crate) stderr: String,
}

impl Finished {
    /// The messages of the log lines.
    pub(crate) fn messages(&self) -> Vec<&str> {
        self.lines
            .iter()
            .map(|line| line["message"].as_str().unwrap())
            .collect()
    }
}

/// Starts `service serve` on `127.0.0.1` with an ephemeral port and waits for its `listening`
/// entry. `variables` complete the configuration (`DATABASE_URL`, `AWS_REGION`, ..).
pub(crate) fn serve(scratch: &Scratch, variables: &[(&str, &str)]) -> Server {
    let mut all = vec![("APP_PORT", "0"), ("APP_HOST", "127.0.0.1")];
    all.extend_from_slice(variables);
    let mut child = scratch
        .service(&["serve"], &all)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = lines_of(BufReader::new(child.stdout.take().unwrap()));
    let stderr = child.stderr.take().unwrap();
    let mut server = Server {
        child,
        stdout,
        stderr,
        port: 0,
        lines: Vec::new(),
    };
    let listening = server.read_until("listening").pop().unwrap();
    server.port = listening["addr"]
        .as_str()
        .unwrap()
        .rsplit(':')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    server
}

impl Server {
    /// Reads log lines up to and including the first one with this message.
    pub(crate) fn read_until(&mut self, message: &str) -> Vec<Value> {
        let start = self.lines.len();
        loop {
            let line = self
                .stdout
                .recv_timeout(LINE_TIMEOUT)
                .unwrap_or_else(|error| {
                    panic!("{message} was not logged ({error}): {:?}", self.lines)
                });
            let json: Value = serde_json::from_str(&line).unwrap();
            let found = json["message"] == message;
            self.lines.push(json);
            if found {
                return self.lines[start..].to_vec();
            }
        }
    }

    /// Sends one request and reads the whole answer (the connection is closed after it).
    #[expect(
        clippy::disallowed_methods,
        reason = "integration tests open the sockets their subject talks to"
    )]
    pub(crate) fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Answer {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let extra = headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join("\r\n");
        let extra = if extra.is_empty() {
            extra
        } else {
            format!("{extra}\r\n")
        };
        let length = body.len();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n{extra}content-length: {length}\r\n\r\n{body}"
        );
        stream.write_all(head.as_bytes()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        Answer::parse(&raw)
    }

    /// Sends a `GET`.
    pub(crate) fn get(&self, path: &str) -> Answer {
        self.request("GET", path, &[], "")
    }

    /// Opens a connection and sends a request head without its final blank line: the request is
    /// in flight until the caller completes it.
    #[expect(
        clippy::disallowed_methods,
        reason = "integration tests open the sockets their subject talks to"
    )]
    pub(crate) fn begin(&self, path: &str) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n"
        )
        .unwrap();
        stream.flush().unwrap();
        stream
    }

    /// Whether a new connection is refused.
    #[expect(
        clippy::disallowed_methods,
        reason = "integration tests open the sockets their subject talks to"
    )]
    pub(crate) fn refuses_connections(&self) -> bool {
        TcpStream::connect(("127.0.0.1", self.port))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused)
    }

    /// Sends a signal (`TERM` or `INT`) with `kill`.
    pub(crate) fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .args([format!("-{name}"), self.child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// Waits until a new connection is refused, for at most `limit`.
    pub(crate) fn wait_until_refusing(&self, limit: Duration) {
        let started = tokio::time::Instant::now();
        while !self.refuses_connections() {
            assert!(started.elapsed() < limit, "connections are still accepted");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Sends SIGTERM and waits for the exit.
    pub(crate) fn terminate(self) -> Finished {
        self.signal("TERM");
        self.finish()
    }

    /// Waits for the exit and collects the exit status, the remaining lines and standard error.
    pub(crate) fn finish(mut self) -> Finished {
        let status = self.child.wait().unwrap();
        let mut lines = std::mem::take(&mut self.lines);
        while let Ok(line) = self.stdout.recv_timeout(LINE_TIMEOUT) {
            lines.push(serde_json::from_str::<Value>(&line).unwrap());
        }
        let mut stderr = String::new();
        self.stderr.read_to_string(&mut stderr).unwrap();
        Finished {
            code: status.code(),
            lines,
            stderr,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Reached with a running child only when a test failed before it ended the process.
        if self.child.try_wait().unwrap_or(None).is_none() {
            let _killed = self.child.kill();
            let _reaped = self.child.wait();
        }
    }
}
