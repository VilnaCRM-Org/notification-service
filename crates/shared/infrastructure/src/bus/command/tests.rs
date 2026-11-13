use std::any::TypeId;
use std::sync::{Arc, Mutex};

use shared_domain::{BusError, Command, CommandBus, CommandHandler, ErasedBusError};

use super::InMemoryCommandBus;
use crate::bus::DuplicateHandler;

#[derive(Debug)]
struct Rename {
    name: String,
}

#[derive(Debug)]
struct Archive;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("name refused")]
struct NameRefused;

impl Command for Rename {
    type Error = NameRefused;
    const NAME: &'static str = "Rename";
}

impl Command for Archive {
    type Error = NameRefused;
    const NAME: &'static str = "Archive";
}

#[derive(Debug, Clone, Default)]
struct RenameHandler {
    seen: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl CommandHandler<Rename> for RenameHandler {
    async fn handle(&self, command: Rename) -> Result<(), NameRefused> {
        self.seen.lock().unwrap().push(command.name.clone());
        if command.name.is_empty() {
            Err(NameRefused)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct ArchiveHandler;

#[async_trait::async_trait]
impl CommandHandler<Archive> for ArchiveHandler {
    async fn handle(&self, _command: Archive) -> Result<(), NameRefused> {
        Ok(())
    }
}

fn rename(name: &str) -> Rename {
    Rename {
        name: name.to_owned(),
    }
}

fn bus_with(handler: RenameHandler) -> Arc<dyn CommandBus> {
    Arc::new(InMemoryCommandBus::new().register(handler).unwrap())
}

#[tokio::test]
async fn delivers_a_registered_command_to_exactly_its_handler() {
    let handler = RenameHandler::default();
    let bus = InMemoryCommandBus::new()
        .register(handler.clone())
        .unwrap()
        .register(ArchiveHandler)
        .unwrap();
    let bus: Arc<dyn CommandBus> = Arc::new(bus);

    bus.dispatch(rename("ada")).await.unwrap();
    bus.dispatch(Archive).await.unwrap();

    assert_eq!(*handler.seen.lock().unwrap(), vec!["ada".to_owned()]);
}

#[tokio::test]
async fn returns_the_typed_error_of_the_handler() {
    let bus = bus_with(RenameHandler::default());

    let result = bus.dispatch(rename("")).await;

    assert!(matches!(result, Err(BusError::Handler(NameRefused))));
}

#[tokio::test]
async fn names_an_unregistered_command() {
    let bus = bus_with(RenameHandler::default());

    let error = bus.dispatch(Archive).await.unwrap_err();

    assert!(matches!(
        error,
        BusError::NotRegistered { message: "Archive" }
    ));
}

#[tokio::test]
async fn reports_a_message_of_another_type_under_a_registered_type_id() {
    let bus = InMemoryCommandBus::new()
        .register(RenameHandler::default())
        .unwrap();

    let result = bus
        .dispatch_erased(TypeId::of::<Rename>(), "Rename", Box::new(Archive))
        .await;

    assert!(matches!(
        result,
        Err(ErasedBusError::NotRegistered { message: "Rename" })
    ));
}

#[test]
fn refuses_a_second_handler_for_the_same_command() {
    let bus = InMemoryCommandBus::new()
        .register(RenameHandler::default())
        .unwrap();

    let error = bus.register(RenameHandler::default()).unwrap_err();

    assert_eq!(error, DuplicateHandler { message: "Rename" });
}

#[test]
fn debug_names_the_type_and_counts_the_handlers() {
    let empty = InMemoryCommandBus::new();
    let one = InMemoryCommandBus::new()
        .register(RenameHandler::default())
        .unwrap();

    assert_eq!(format!("{empty:?}"), "InMemoryCommandBus { handlers: 0 }");
    assert_eq!(format!("{one:?}"), "InMemoryCommandBus { handlers: 1 }");
}
