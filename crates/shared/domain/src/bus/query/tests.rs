use std::any::TypeId;

use super::{Query, QueryBus, QueryHandler};
use crate::bus::error::{BusError, ErasedBusError, ErasedMessage};

#[derive(Debug)]
struct NameOf {
    id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("lookup failed")]
struct LookupFailed;

impl Query for NameOf {
    type Output = String;
    type Error = LookupFailed;
    const NAME: &'static str = "NameOf";
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Answer,
    Fail,
    WrongErrorType,
    WrongOutputType,
    Missing,
}

#[derive(Debug)]
struct StubHandler;

impl StubHandler {
    fn name_of(query: &NameOf) -> Result<String, LookupFailed> {
        if query.id == 1 {
            Ok("ada".to_owned())
        } else {
            Err(LookupFailed)
        }
    }
}

#[async_trait::async_trait]
impl QueryHandler<NameOf> for StubHandler {
    async fn handle(&self, query: NameOf) -> Result<String, LookupFailed> {
        Self::name_of(&query)
    }
}

#[derive(Debug)]
struct StubBus {
    mode: Mode,
}

impl StubBus {
    async fn ask_handler(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        match self.mode {
            Mode::Missing => Err(ErasedBusError::NotRegistered { message: name }),
            Mode::WrongErrorType => Err(ErasedBusError::Handler(Box::new("not a handler error"))),
            Mode::WrongOutputType => Ok(Box::new(7_u8)),
            Mode::Answer | Mode::Fail => {
                assert_eq!(query_type, TypeId::of::<NameOf>());
                let mut query = query.downcast::<NameOf>().unwrap();
                if matches!(self.mode, Mode::Fail) {
                    query.id = 2;
                }
                let output = StubHandler.handle(*query).await;
                output
                    .map(|output| -> ErasedMessage { Box::new(output) })
                    .map_err(|error| ErasedBusError::Handler(Box::new(error)))
            }
        }
    }
}

#[async_trait::async_trait]
impl QueryBus for StubBus {
    async fn ask_erased(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        self.ask_handler(query_type, name, query).await
    }
}

#[tokio::test]
async fn returns_the_handler_output_with_its_static_type() {
    let bus = StubBus { mode: Mode::Answer };
    let bus: &dyn QueryBus = &bus;

    let output: String = bus.ask(NameOf { id: 1 }).await.unwrap();

    assert_eq!(output, "ada");
}

#[tokio::test]
async fn delivers_a_handler_error_typed_without_a_downcast() {
    let bus = StubBus { mode: Mode::Fail };
    let bus: &dyn QueryBus = &bus;

    let result = bus.ask(NameOf { id: 1 }).await;

    assert!(matches!(result, Err(BusError::Handler(LookupFailed))));
}

#[tokio::test]
async fn names_the_query_when_no_handler_is_registered() {
    let bus = StubBus {
        mode: Mode::Missing,
    };
    let bus: &dyn QueryBus = &bus;

    let error = bus.ask(NameOf { id: 1 }).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "NameOf" }
    ));
    assert_eq!(error.to_string(), "no handler is registered for `NameOf`");
}

#[tokio::test]
async fn reports_a_missing_handler_when_the_erased_error_has_another_type() {
    let bus = StubBus {
        mode: Mode::WrongErrorType,
    };
    let bus: &dyn QueryBus = &bus;

    let error = bus.ask(NameOf { id: 1 }).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "NameOf" }
    ));
}

#[tokio::test]
async fn reports_a_missing_handler_when_the_erased_output_has_another_type() {
    let bus = StubBus {
        mode: Mode::WrongOutputType,
    };
    let bus: &dyn QueryBus = &bus;

    let error = bus.ask(NameOf { id: 1 }).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "NameOf" }
    ));
}

#[test]
fn a_query_carries_its_name() {
    assert_eq!(NameOf::NAME, "NameOf");
}
