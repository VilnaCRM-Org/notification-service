//! Integration tests of the service binary: it is spawned as a process, with a cleared environment.

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod broker;
#[cfg(test)]
mod check_config;
#[cfg(test)]
mod client;
#[cfg(test)]
mod cluster;
#[cfg(test)]
mod export;
#[cfg(test)]
mod graphql_sse;
#[cfg(test)]
mod healthcheck;
#[cfg(test)]
mod limit;
#[cfg(test)]
mod panic;
#[cfg(test)]
mod request_target;
#[cfg(test)]
mod serve;
#[cfg(test)]
mod serve_stream;
#[cfg(test)]
mod support;
