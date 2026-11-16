//! Integration tests of the in-process adapters that need real time, and of the SQS adapter that needs the
//! test stack.

#[cfg(test)]
mod cache;
#[cfg(test)]
mod clock;
#[cfg(test)]
mod id;
#[cfg(test)]
mod messaging;
#[cfg(test)]
mod support;
