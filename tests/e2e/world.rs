//! The world of a scenario: injected faults, the last response, the created ids and the scenario
//! database. A new application instance is built for every request.

use std::collections::BTreeSet;
use std::str::FromStr as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, Request, header};
use clap::Parser as _;
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use serde_json::Value;
use service::bootstrap::{Adapters, router};
use service::cli::{Cli, Command};
use service::config::Config;
use shared_infrastructure::persistence::MIGRATOR;
use sqlx::postgres::PgConnectOptions;
use sqlx::{AssertSqlSafe, ConnectOptions as _, Connection as _, PgConnection};
use test_support::probe::StubProbe;
use tower::ServiceExt as _;

/// Counter that makes the database names of the scenarios of one process unique.
static DATABASES: AtomicUsize = AtomicUsize::new(0);

/// The token that stands for the remembered id in paths, doc strings and string arguments.
const ID_TOKEN: &str = "{id}";

/// What the last request answered.
#[derive(Debug)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl Reply {
    /// The body as JSON, `Null` when it is not JSON.
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// One instance per scenario; cucumber builds it with `Default`.
#[derive(Debug, Default, cucumber::World)]
pub(crate) struct AppWorld {
    failing: Vec<Dependency>,
    database: Option<String>,
    reply: Option<Reply>,
    created: Vec<String>,
    remembered: Option<String>,
}

/// The configuration as the binary reads it from the environment.
fn base_config() -> Config {
    let Command::Serve(args) = Cli::parse_from(["service", "serve"]).command else {
        panic!("`serve` arguments expected");
    };
    let mut config = Config::from_args(args).expect("valid configuration");
    // The production defaults are a few seconds. A scenario must succeed however long the machine
    // needs to open the first connection, so the budgets are long and keep the order the
    // configuration requires (request > probe >= acquire and operation).
    config.database.acquire_timeout = Duration::from_secs(30);
    config.sqs.connect_timeout = Duration::from_secs(10);
    config.sqs.operation_timeout = Duration::from_secs(30);
    config.health_probe_timeout = Duration::from_secs(40);
    config.request_timeout = Duration::from_secs(60);
    config
}

/// Connection options of the server of `DATABASE_URL`.
fn server_options() -> PgConnectOptions {
    PgConnectOptions::from_str(&base_config().database.url).expect("valid database URL")
}

/// A connection to the server of `DATABASE_URL`, used to create and drop databases.
async fn admin() -> PgConnection {
    server_options()
        .connect()
        .await
        .expect("the database server is reachable")
}

/// Drops every database a killed run left behind (and, with it, any other `e2e_` database: two
/// runs must not share a server at the same time).
pub(crate) async fn drop_leftover_databases() {
    let mut connection = admin().await;
    let names: Vec<String> =
        sqlx::query_scalar("SELECT datname FROM pg_database WHERE datname LIKE 'e2e\\_%'")
            .fetch_all(&mut connection)
            .await
            .expect("list the leftover databases");
    for name in names {
        drop_named(&mut connection, &name).await;
    }
}

async fn drop_named(connection: &mut PgConnection, name: &str) {
    let statement = format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)");
    sqlx::query(AssertSqlSafe(statement))
        .execute(connection)
        .await
        .expect("drop the database");
}

impl AppWorld {
    /// Marks a dependency as failing: its probe is replaced before the next request.
    pub(crate) fn fail(&mut self, dependency: Dependency) {
        self.failing.push(dependency);
    }

    /// The last response.
    pub(crate) fn reply(&self) -> &Reply {
        self.reply.as_ref().expect("a request was sent")
    }

    /// The ids of the resources the scenario created.
    pub(crate) fn created(&self) -> BTreeSet<String> {
        self.created.iter().cloned().collect()
    }

    /// Asserts that the ids of the JSON array are exactly the created ids, and that there are some.
    pub(crate) fn assert_exactly_created(&self, array: &Value) {
        let ids: BTreeSet<String> = array
            .as_array()
            .expect("a JSON array")
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        let created = self.created();
        assert!(!created.is_empty(), "the scenario created no resource");
        assert_eq!(ids, created);
    }

    /// Replaces the `{id}` token by the remembered id.
    pub(crate) fn substitute(&self, text: &str) -> String {
        self.remembered
            .as_deref()
            .map_or_else(|| text.to_owned(), |id| text.replace(ID_TOKEN, id))
    }

    /// Creates the scenario database on first use and returns its URL.
    async fn database_url(&mut self) -> String {
        let options = server_options();
        if self.database.is_none() {
            let name = format!(
                "e2e_{}_{}",
                std::process::id(),
                DATABASES.fetch_add(1, Ordering::SeqCst)
            );
            let statement = format!("CREATE DATABASE \"{name}\"");
            sqlx::query(AssertSqlSafe(statement))
                .execute(&mut admin().await)
                .await
                .expect("create the scenario database");
            let mut connection = options.clone().database(&name).connect().await.unwrap();
            MIGRATOR.run(&mut connection).await.expect("migrate");
            connection.close().await.expect("close");
            self.database = Some(name);
        }
        url_with_database(self.database.as_deref().expect("created above"))
    }

    /// Drops the scenario database, if the scenario made one.
    pub(crate) async fn drop_database(&mut self) {
        if let Some(name) = self.database.take() {
            drop_named(&mut admin().await, &name).await;
        }
    }

    /// Builds a new application instance and sends one request to it.
    pub(crate) async fn send(
        &mut self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: &str,
    ) {
        let url = self.database_url().await;
        let mut config = base_config();
        config.database.url = url;
        let mut adapters = Adapters::connect(&config).await.expect("production wiring");
        for dependency in &self.failing {
            adapters = adapters.with_probe(Arc::new(StubProbe::failing(*dependency)));
        }
        let mut request = Request::builder().method(method).uri(self.substitute(path));
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        let request = request
            .body(Body::from(self.substitute(body)))
            .expect("valid request");
        let response = router(adapters, &config)
            .expect("assembled router")
            .oneshot(request)
            .await
            .expect("infallible");
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let reply = Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        };
        self.record(&reply);
        self.reply = Some(reply);
    }

    /// Remembers the id of a created resource: a 201 body or a GraphQL mutation result.
    fn record(&mut self, reply: &Reply) {
        let json = reply.json();
        let created = if reply.status == 201 {
            json.get("id")
        } else {
            json.pointer("/data/createExampleApiResource/id")
        };
        if let Some(id) = created.and_then(Value::as_str) {
            self.created.push(id.to_owned());
            self.remembered = Some(id.to_owned());
        }
    }
}

/// The URL of database `name` on the server of `DATABASE_URL`: the path segment is replaced.
fn url_with_database(name: &str) -> String {
    let url = base_config().database.url;
    let (server, database) = url.rsplit_once('/').expect("a database name");
    let query = database
        .split_once('?')
        .map_or_else(String::new, |(_, query)| format!("?{query}"));
    format!("{server}/{name}{query}")
}
