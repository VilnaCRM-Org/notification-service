use std::any::TypeId;
use std::sync::Arc;

use shared_domain::{BusError, ErasedBusError, Query, QueryBus, QueryHandler};

use super::InMemoryQueryBus;
use crate::bus::DuplicateHandler;

#[derive(Debug)]
struct Lookup {
    id: u32,
}

#[derive(Debug)]
struct Count;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not found")]
struct NotFound;

impl Query for Lookup {
    type Output = String;
    type Error = NotFound;
    const NAME: &'static str = "Lookup";
}

impl Query for Count {
    type Output = u32;
    type Error = NotFound;
    const NAME: &'static str = "Count";
}

#[derive(Debug)]
struct LookupHandler;

#[async_trait::async_trait]
impl QueryHandler<Lookup> for LookupHandler {
    async fn handle(&self, query: Lookup) -> Result<String, NotFound> {
        if query.id == 7 {
            Ok("seven".to_owned())
        } else {
            Err(NotFound)
        }
    }
}

#[derive(Debug)]
struct CountHandler;

#[async_trait::async_trait]
impl QueryHandler<Count> for CountHandler {
    async fn handle(&self, _query: Count) -> Result<u32, NotFound> {
        Ok(3)
    }
}

fn bus() -> Arc<dyn QueryBus> {
    Arc::new(InMemoryQueryBus::new().register(LookupHandler).unwrap())
}

#[tokio::test]
async fn answers_a_registered_query_with_the_output_of_its_handler() {
    let bus: Arc<dyn QueryBus> = Arc::new(
        InMemoryQueryBus::new()
            .register(LookupHandler)
            .unwrap()
            .register(CountHandler)
            .unwrap(),
    );

    assert_eq!(bus.ask(Lookup { id: 7 }).await.unwrap(), "seven");
    assert_eq!(bus.ask(Count).await.unwrap(), 3);
}

#[tokio::test]
async fn returns_the_typed_error_of_the_handler() {
    let result = bus().ask(Lookup { id: 1 }).await;

    assert!(matches!(result, Err(BusError::Handler(NotFound))));
}

#[tokio::test]
async fn names_an_unregistered_query() {
    let error = bus().ask(Count).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "Count" }
    ));
}

#[tokio::test]
async fn reports_a_message_of_another_type_under_a_registered_type_id() {
    let bus = InMemoryQueryBus::new().register(LookupHandler).unwrap();

    let result = bus
        .ask_erased(TypeId::of::<Lookup>(), "Lookup", Box::new(Count))
        .await;

    assert!(matches!(
        result,
        Err(ErasedBusError::NotRegistered { message: "Lookup" })
    ));
}

#[test]
fn refuses_a_second_handler_for_the_same_query() {
    let bus = InMemoryQueryBus::new().register(LookupHandler).unwrap();

    let error = bus.register(LookupHandler).unwrap_err();

    assert_eq!(error, DuplicateHandler { message: "Lookup" });
}

#[test]
fn debug_names_the_type_and_counts_the_handlers() {
    let empty = InMemoryQueryBus::new();
    let one = InMemoryQueryBus::new().register(LookupHandler).unwrap();

    assert_eq!(format!("{empty:?}"), "InMemoryQueryBus { handlers: 0 }");
    assert_eq!(format!("{one:?}"), "InMemoryQueryBus { handlers: 1 }");
}
