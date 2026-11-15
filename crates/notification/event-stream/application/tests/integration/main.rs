//! Integration tests of the WebSocket route of the event stream over a real socket: the routes of
//! the context served on a loopback port of their own per test, and a tungstenite client.

#[cfg(test)]
mod support;
#[cfg(test)]
mod ws;
