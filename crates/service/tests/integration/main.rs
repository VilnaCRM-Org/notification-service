//! Integration tests of the service binary: it is spawned as a process, with a cleared environment.

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod check_config;
#[cfg(test)]
mod export;
#[cfg(test)]
mod healthcheck;
#[cfg(test)]
mod migrate;
#[cfg(test)]
mod panic;
#[cfg(test)]
mod serve;
#[cfg(test)]
mod support;
