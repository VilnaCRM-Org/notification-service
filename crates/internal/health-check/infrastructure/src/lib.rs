//! Health-check context, infrastructure layer: the probe that checks the cache and implements the
//! `HealthProbe` port of the domain crate.
//!
//! This crate never names an application crate: the probes are wired in by the composition root.

pub mod probe;

pub use probe::cache::CacheProbe;
