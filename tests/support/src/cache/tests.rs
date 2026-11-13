use std::time::Duration;

use shared_domain::Cache;

use super::StubCache;
use crate::block_on;

const TTL: Duration = Duration::from_secs(60);

#[test]
fn the_working_cache_returns_what_was_stored() {
    let cache = StubCache::working();
    let port: &dyn Cache = &cache;

    assert_eq!(block_on(port.get("k")).unwrap(), None);
    block_on(port.set("k", b"v".to_vec(), TTL)).unwrap();

    assert_eq!(block_on(port.get("k")).unwrap(), Some(b"v".to_vec()));
    assert_eq!(block_on(port.get("other")).unwrap(), None);
}

#[test]
fn the_failing_cache_errors_on_every_call() {
    let cache = StubCache::failing();
    let port: &dyn Cache = &cache;

    assert_eq!(
        block_on(port.set("k", b"v".to_vec(), TTL))
            .unwrap_err()
            .to_string(),
        "cache operation failed"
    );
    assert!(block_on(port.get("k")).is_err());
}

#[test]
fn the_forgetful_cache_accepts_writes_and_returns_nothing() {
    let cache = StubCache::forgetful();
    let port: &dyn Cache = &cache;

    block_on(port.set("k", b"v".to_vec(), TTL)).unwrap();

    assert_eq!(block_on(port.get("k")).unwrap(), None);
}

#[test]
fn the_corrupting_cache_returns_a_different_value() {
    let cache = StubCache::corrupting();
    let port: &dyn Cache = &cache;

    block_on(port.set("k", b"v".to_vec(), TTL)).unwrap();
    block_on(port.set("empty", Vec::new(), TTL)).unwrap();

    let value = block_on(port.get("k")).unwrap().unwrap();
    let empty = block_on(port.get("empty")).unwrap().unwrap();
    assert_ne!(value, b"v".to_vec());
    assert_ne!(empty, Vec::<u8>::new());
    assert_eq!(block_on(port.get("missing")).unwrap(), None);
}

#[test]
fn the_cache_is_debug() {
    assert!(format!("{:?}", StubCache::working()).contains("StubCache"));
}
