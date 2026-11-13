use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use shared_domain::{BoxError, DomainEvent, DomainEventSubscriber, EventBus, Uuid};

use super::{ErasedSubscriber as _, EventTypeMismatch, InMemoryEventBus, SubscriberAdapter};

#[derive(Debug)]
struct OrderPlaced {
    id: u128,
}

#[derive(Debug)]
struct OrderPaid;

impl DomainEvent for OrderPlaced {
    fn event_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::from_u128(self.id))
    }

    fn aggregate_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::nil())
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    fn event_name(&self) -> &'static str {
        "order.placed"
    }
}

impl DomainEvent for OrderPaid {
    fn event_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::nil())
    }

    fn aggregate_id(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::nil())
    }

    fn occurred_at(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    fn event_name(&self) -> &'static str {
        "order.paid"
    }
}

#[derive(Debug, thiserror::Error)]
#[error("subscriber {0} failed")]
struct Failed(&'static str);

type Log = Arc<Mutex<Vec<String>>>;

struct Recorder {
    label: &'static str,
    fails: bool,
    log: Log,
}

impl Recorder {
    fn new(label: &'static str, fails: bool, log: &Log) -> Self {
        Self {
            label,
            fails,
            log: Arc::clone(log),
        }
    }
}

#[async_trait::async_trait]
impl DomainEventSubscriber<OrderPlaced> for Recorder {
    async fn handle(&self, event: &OrderPlaced) -> Result<(), BoxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:{}", self.label, event.id));
        if self.fails {
            Err(Box::new(Failed(self.label)))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl DomainEventSubscriber<OrderPaid> for Recorder {
    async fn handle(&self, _event: &OrderPaid) -> Result<(), BoxError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("{}:paid", self.label));
        Ok(())
    }
}

fn placed(id: u128) -> Box<dyn DomainEvent> {
    Box::new(OrderPlaced { id })
}

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

#[tokio::test]
async fn runs_every_subscriber_in_registration_order_and_returns_every_failure() {
    let log = Log::default();
    let bus = InMemoryEventBus::new()
        .subscribe::<OrderPlaced, _>(Recorder::new("first", true, &log))
        .subscribe::<OrderPlaced, _>(Recorder::new("second", true, &log))
        .subscribe::<OrderPlaced, _>(Recorder::new("third", false, &log));
    let bus: Arc<dyn EventBus> = Arc::new(bus);

    let error = bus.publish(vec![placed(5)]).await.unwrap_err();

    assert_eq!(entries(&log), ["first:5", "second:5", "third:5"]);
    assert_eq!(error.failures.len(), 2);
    assert_eq!(error.failures[0].event_name, "order.placed");
    assert_eq!(
        error.failures[0].event_id.to_string(),
        "00000000-0000-0000-0000-000000000005"
    );
    assert_eq!(
        error.failures[0].source.to_string(),
        "subscriber first failed"
    );
    assert_eq!(error.failures[1].event_name, "order.placed");
    assert_eq!(error.failures[1].event_id, error.failures[0].event_id);
    assert_eq!(
        error.failures[1].source.to_string(),
        "subscriber second failed"
    );
}

#[tokio::test]
async fn succeeds_when_every_subscriber_succeeds() {
    let log = Log::default();
    let bus =
        InMemoryEventBus::new().subscribe::<OrderPlaced, _>(Recorder::new("only", false, &log));

    bus.publish(vec![placed(1), placed(2)]).await.unwrap();

    assert_eq!(entries(&log), ["only:1", "only:2"]);
}

#[tokio::test]
async fn succeeds_when_the_event_has_no_subscriber() {
    let log = Log::default();
    let bus =
        InMemoryEventBus::new().subscribe::<OrderPlaced, _>(Recorder::new("only", false, &log));

    assert!(bus.publish(vec![Box::new(OrderPaid)]).await.is_ok());
    assert!(
        InMemoryEventBus::new()
            .publish(vec![placed(1)])
            .await
            .is_ok()
    );
    assert!(InMemoryEventBus::new().publish(Vec::new()).await.is_ok());
    assert_eq!(entries(&log), Vec::<String>::new());
}

#[tokio::test]
async fn keeps_the_failures_of_several_events_in_invocation_order() {
    let log = Log::default();
    let bus = InMemoryEventBus::new()
        .subscribe::<OrderPlaced, _>(Recorder::new("a", true, &log))
        .subscribe::<OrderPaid, _>(Recorder::new("b", false, &log));

    let error = bus
        .publish(vec![placed(1), Box::new(OrderPaid), placed(2)])
        .await
        .unwrap_err();

    assert_eq!(entries(&log), ["a:1", "b:paid", "a:2"]);
    let ids: Vec<String> = error
        .failures
        .iter()
        .map(|failure| failure.event_id.to_string())
        .collect();
    assert_eq!(
        ids,
        [
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002"
        ]
    );
}

#[tokio::test]
async fn reports_an_event_of_another_type_handed_to_a_subscriber() {
    let log = Log::default();
    let adapter: SubscriberAdapter<OrderPlaced, Recorder> = SubscriberAdapter {
        subscriber: Recorder::new("only", false, &log),
        event: std::marker::PhantomData,
    };

    let error = adapter.handle(&OrderPaid).await.unwrap_err();

    assert!(error.is::<EventTypeMismatch>());
    assert!(error.to_string().contains("OrderPlaced"));
    assert_eq!(entries(&log), Vec::<String>::new());
}

#[test]
fn debug_names_the_type_and_counts_the_event_types() {
    let log = Log::default();
    let empty = InMemoryEventBus::new();
    let two = InMemoryEventBus::new()
        .subscribe::<OrderPlaced, _>(Recorder::new("a", false, &log))
        .subscribe::<OrderPlaced, _>(Recorder::new("b", false, &log))
        .subscribe::<OrderPaid, _>(Recorder::new("c", false, &log));

    assert_eq!(format!("{empty:?}"), "InMemoryEventBus { event_types: 0 }");
    assert_eq!(format!("{two:?}"), "InMemoryEventBus { event_types: 2 }");
}
