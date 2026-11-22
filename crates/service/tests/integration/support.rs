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

/// Every variable of a valid configuration that has no default: the Kafka brokers.
pub(crate) const VALID_ENVIRONMENT: [(&str, &str); 1] =
    [("KAFKA_BOOTSTRAP_SERVERS", "localhost:9092")];

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

/// A listener on a free loopback port that accepts connections (the kernel does) and never
/// answers: the peer connects and then waits.
#[expect(
    clippy::disallowed_methods,
    reason = "integration tests open the sockets their subject talks to"
)]
pub(crate) fn silent_listener() -> std::net::TcpListener {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap()
}

/// A variable of the test stack (`KAFKA_BOOTSTRAP_SERVERS` and the like), taken from the
/// environment of the test process.
pub(crate) fn stack_variable(name: &str) -> String {
    from_environment(name)
        .unwrap_or_else(|| panic!("{name} is not set; run the tests through `make`"))
        .into_string()
        .unwrap()
}

/// The variables that let `serve` start against the compose services: the Kafka broker of the
/// test stack (its topic `notifications` is the default), with long budgets so that a request that
/// must succeed does not depend on how fast a loaded machine answers (request > probe, as the
/// configuration requires).
pub(crate) fn stack() -> Vec<(&'static str, String)> {
    vec![
        (
            "KAFKA_BOOTSTRAP_SERVERS",
            stack_variable("KAFKA_BOOTSTRAP_SERVERS"),
        ),
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
    /// The answer whose raw bytes, read up to the end of the connection, are `raw`.
    pub(crate) fn parse(raw: &str) -> Self {
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        Self {
            status,
            head: head.to_lowercase(),
            body: body.to_owned(),
        }
    }
}

/// The messages of the start-up entry that reports the open file limit: raised, or kept where the
/// machine does not allow raising it. The entry comes before `listening`, and its values depend on
/// the machine, so the log of a process is read without it.
const FILE_LIMIT_MESSAGES: [&str; 2] = ["open file limit", "open file limit not raised"];

/// Whether `line` is the start-up entry of the open file limit.
pub(crate) fn is_file_limit(line: &Value) -> bool {
    line["message"]
        .as_str()
        .is_some_and(|message| FILE_LIMIT_MESSAGES.contains(&message))
}

/// The log lines with `message`.
pub(crate) fn messages_of<'a>(lines: &'a [Value], message: &str) -> Vec<&'a Value> {
    lines
        .iter()
        .filter(|line| line["message"] == message)
        .collect()
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
    /// Every log line read so far but the entry of the open file limit.
    pub(crate) lines: Vec<Value>,
}

/// How long a test waits for the next line of a service before it fails.
const LINE_TIMEOUT: Duration = Duration::from_secs(20);

/// The request id of the requests that wait for readiness, so that a test can tell their log
/// entries from those of the requests it checks.
pub(crate) const READINESS: &str = "readiness-wait";

/// How long a test waits for a service to become ready.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// Every log line of the process but the entry of the open file limit, in order.
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
/// entry. `variables` complete the configuration (`KAFKA_BOOTSTRAP_SERVERS`, ..).
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
            if is_file_limit(&json) {
                continue;
            }
            let found = json["message"] == message;
            self.lines.push(json);
            if found {
                return self.lines[start..].to_vec();
            }
        }
    }

    /// Sends one request and reads the whole answer (the connection is closed after it).
    pub(crate) fn request(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Answer {
        request_to(self.port, method, path, headers, body)
    }

    /// Sends a `GET`.
    pub(crate) fn get(&self, path: &str) -> Answer {
        self.request("GET", path, &[], "")
    }

    /// Polls `GET /api/events/status` (request id [`READINESS`]) until the node reports `ready`,
    /// for at most 10 s: the probe of the event stream passes from then on. The blocking poll runs
    /// on the blocking pool, so the runtime of the calling test stays free; the future holds the
    /// port only, not the server, whose receiver of log lines is not `Sync`.
    pub(crate) fn wait_until_ready(&self) -> impl Future<Output = ()> + Send + use<> {
        let port = self.port;
        async move {
            tokio::task::spawn_blocking(move || poll_until_ready(port))
                .await
                .unwrap_or_else(|failed| std::panic::resume_unwind(failed.into_panic()));
        }
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
            let json = serde_json::from_str::<Value>(&line).unwrap();
            if !is_file_limit(&json) {
                lines.push(json);
            }
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

/// Sends one request to the service on `port` and reads the whole answer (the connection is
/// closed after it). Every write and every read waits at most [`LINE_TIMEOUT`], so a service that
/// accepts the connection and never answers fails the test instead of hanging it.
#[expect(
    clippy::disallowed_methods,
    reason = "integration tests open the sockets their subject talks to"
)]
fn request_to(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Answer {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_write_timeout(Some(LINE_TIMEOUT)).unwrap();
    stream.set_read_timeout(Some(LINE_TIMEOUT)).unwrap();
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
    stream
        .write_all(head.as_bytes())
        .unwrap_or_else(|error| panic!("{method} {path} was not sent: {error}"));
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .unwrap_or_else(|error| panic!("{method} {path} was not answered: {error}"));
    Answer::parse(&raw)
}

/// Polls `GET /api/events/status` of the service on `port` (request id [`READINESS`]) until the
/// node reports `ready`, for at most [`READY_TIMEOUT`]; each request is bounded by [`request_to`].
fn poll_until_ready(port: u16) {
    let started = tokio::time::Instant::now();
    loop {
        let answer = request_to(
            port,
            "GET",
            "/api/events/status",
            &[("x-request-id", READINESS)],
            "",
        );
        if answer.body.contains(r#""state":"ready""#) {
            return;
        }
        assert!(
            started.elapsed() < READY_TIMEOUT,
            "the node did not become ready: {}",
            answer.body
        );
        std::thread::sleep(Duration::from_millis(50));
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
