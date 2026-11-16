use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use event_stream_domain::EventType;
use rstest::rstest;
use shared_domain::{InvalidPageRequest, Query as _, QueryHandler as _};
use tokio::time::timeout;

use super::{ListEventTypes, ListEventTypesHandler};
use crate::catalog::EventCatalog;
use crate::error::EventStreamError;
use crate::query::EventTypeView;

/// The bound of every awaited call of these tests.
const WAIT: Duration = Duration::from_secs(5);

fn handler() -> ListEventTypesHandler {
    let types = ["user.deleted", "order.paid", "user.created"]
        .into_iter()
        .map(|name| EventType::new(name.to_owned()).unwrap())
        .collect::<BTreeSet<_>>();
    ListEventTypesHandler::new(Arc::new(EventCatalog::new(types)))
}

fn views(names: &[&str]) -> Vec<EventTypeView> {
    names
        .iter()
        .map(|name| EventTypeView {
            name: (*name).to_owned(),
        })
        .collect()
}

async fn handled(query: ListEventTypes) -> Result<Vec<EventTypeView>, EventStreamError> {
    timeout(WAIT, handler().handle(query))
        .await
        .expect("the handler answers within WAIT")
}

const fn ask(page: u32, items_per_page: u32) -> ListEventTypes {
    ListEventTypes {
        page,
        items_per_page,
    }
}

#[test]
fn the_query_is_named_after_its_type() {
    assert_eq!(ListEventTypes::NAME, "ListEventTypes");
}

#[rstest]
#[case(1, 30, &["order.paid", "user.created", "user.deleted"])]
#[case(2, 1, &["user.created"])]
#[case(4, 1, &[])]
#[case(1, 100, &["order.paid", "user.created", "user.deleted"])]
#[tokio::test]
async fn answers_the_page_of_the_catalog_as_views(
    #[case] page: u32,
    #[case] items_per_page: u32,
    #[case] expected: &[&str],
) {
    let answer = handled(ask(page, items_per_page)).await.unwrap();

    assert_eq!(answer, views(expected));
}

#[rstest]
#[case::page_zero(0, 30, InvalidPageRequest::Page)]
#[case::size_zero(1, 0, InvalidPageRequest::ItemsPerPage)]
#[case::size_above_100(1, 101, InvalidPageRequest::ItemsPerPage)]
#[tokio::test]
async fn an_invalid_page_request_is_invalid_page(
    #[case] page: u32,
    #[case] items_per_page: u32,
    #[case] expected: InvalidPageRequest,
) {
    let error = handled(ask(page, items_per_page)).await.unwrap_err();

    assert!(matches!(error, EventStreamError::InvalidPage(found) if found == expected));
}
