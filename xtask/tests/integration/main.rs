//! Integration tests of the `xtask` binary: it runs as a subprocess, as `cargo xtask` runs it.

#[cfg(test)]
mod arch;
#[cfg(test)]
mod config_guard;
#[cfg(test)]
mod scaffold;
#[cfg(test)]
mod support;
