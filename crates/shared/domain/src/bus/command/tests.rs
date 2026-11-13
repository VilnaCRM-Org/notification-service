use std::any::TypeId;

use super::{Command, CommandBus, CommandHandler};
use crate::bus::error::{BusError, ErasedBusError, ErasedMessage};

#[derive(Debug, PartialEq, Eq)]
struct Rename {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("rename failed")]
struct RenameFailed;

impl Command for Rename {
    type Error = RenameFailed;
    const NAME: &'static str = "Rename";
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Succeed,
    Fail,
    WrongErrorType,
    Missing,
}

#[derive(Debug)]
struct StubHandler {
    mode: Mode,
}

impl StubHandler {
    fn rename(&self, command: &Rename) -> Result<(), RenameFailed> {
        match (self.mode, command.name.as_str()) {
            (Mode::Succeed, "ada") => Ok(()),
            _ => Err(RenameFailed),
        }
    }
}

#[async_trait::async_trait]
impl CommandHandler<Rename> for StubHandler {
    async fn handle(&self, command: Rename) -> Result<(), RenameFailed> {
        self.rename(&command)
    }
}

#[derive(Debug)]
struct StubBus {
    mode: Mode,
}

impl StubBus {
    async fn dispatch_to_handler(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        match self.mode {
            Mode::Missing => Err(ErasedBusError::NotRegistered { message: name }),
            Mode::WrongErrorType => Err(ErasedBusError::Handler(Box::new("not a handler error"))),
            Mode::Succeed | Mode::Fail => {
                assert_eq!(command_type, TypeId::of::<Rename>());
                let command = command.downcast::<Rename>().unwrap();
                StubHandler { mode: self.mode }
                    .handle(*command)
                    .await
                    .map_err(|error| ErasedBusError::Handler(Box::new(error)))
            }
        }
    }
}

#[async_trait::async_trait]
impl CommandBus for StubBus {
    async fn dispatch_erased(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        self.dispatch_to_handler(command_type, name, command).await
    }
}

fn rename(name: &str) -> Rename {
    Rename {
        name: name.to_owned(),
    }
}

#[tokio::test]
async fn returns_the_handler_result_through_a_dyn_bus() {
    let bus = StubBus {
        mode: Mode::Succeed,
    };
    let bus: &dyn CommandBus = &bus;

    let result: Result<(), BusError<RenameFailed>> = bus.dispatch(rename("ada")).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn delivers_a_handler_error_typed_without_a_downcast() {
    let bus = StubBus { mode: Mode::Fail };
    let bus: &dyn CommandBus = &bus;

    let result = bus.dispatch(rename("ada")).await;

    assert!(matches!(result, Err(BusError::Handler(RenameFailed))));
}

#[tokio::test]
async fn names_the_command_when_no_handler_is_registered() {
    let bus = StubBus {
        mode: Mode::Missing,
    };
    let bus: &dyn CommandBus = &bus;

    let error = bus.dispatch(rename("ada")).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "Rename" }
    ));
    assert_eq!(error.to_string(), "no handler is registered for `Rename`");
}

#[tokio::test]
async fn reports_a_missing_handler_when_the_erased_error_has_another_type() {
    let bus = StubBus {
        mode: Mode::WrongErrorType,
    };
    let bus: &dyn CommandBus = &bus;

    let error = bus.dispatch(rename("ada")).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "Rename" }
    ));
}

#[tokio::test]
async fn works_through_a_shared_pointer() {
    let bus: std::sync::Arc<dyn CommandBus> = std::sync::Arc::new(StubBus {
        mode: Mode::Succeed,
    });

    assert!(bus.dispatch(rename("ada")).await.is_ok());
}

#[test]
fn a_command_carries_its_name() {
    assert_eq!(Rename::NAME, "Rename");
}
