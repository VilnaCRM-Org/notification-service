//! Test doubles, fixtures and seeded random data shared by the unit, integration and end-to-end
//! tests. This crate is test code: it is excluded from coverage and mutation by path.

pub mod bus;
pub mod cache;
pub mod clock;
pub mod fixture;
pub mod id;
pub mod log;
pub mod probe;
pub mod repository;
pub mod seed;

/// Runs a future that completes without waiting for anything, such as the doubles of this crate.
#[cfg(test)]
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}
