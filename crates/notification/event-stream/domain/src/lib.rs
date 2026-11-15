//! Bounded context `event-stream`, domain layer: aggregates, value objects, domain events, domain
//! errors and the ports (such as repository traits) that the infrastructure layer implements.
//!
//! This crate is framework-free. It depends on domain crates only: its own context and the shared
//! kernel, never another bounded context.

pub mod error;
pub mod event;
pub mod port;
pub mod value_object;

pub use error::{InvalidCursor, InvalidEvent};
pub use event::StoredEvent;
pub use port::topic::{Batch, TopicError, TopicProbe, TopicRecord, TopicReplay, TopicTail};
pub use value_object::cursor::Cursor;
pub use value_object::event_type::EventType;
pub use value_object::offset::Offset;
pub use value_object::page_limit::PageLimit;
pub use value_object::sequence::Sequence;
pub use value_object::watermarks::Watermarks;
