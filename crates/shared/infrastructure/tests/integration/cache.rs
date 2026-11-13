use std::sync::Arc;
use std::time::Duration;

use shared_domain::Cache;
use shared_infrastructure::cache::MokaCache;
use tokio::time::Instant;

const LONG: Duration = Duration::from_secs(3600);
const SHORT: Duration = Duration::from_millis(150);
const PAST_SHORT: Duration = Duration::from_millis(400);

/// How many times the first test may try to read a value back inside its TTL.
const ATTEMPTS: usize = 100;

/// Stores a value under a fresh key with the TTL `SHORT` and reads it straight back, and returns the
/// key and what was read.
///
/// A read that happened after the TTL says something about the machine and nothing about the
/// cache, so such an attempt is repeated (the window is measured from before the store to after
/// the read). `None` means that no attempt was fast enough.
async fn stored_and_read_back_within_the_ttl(
    cache: &Arc<dyn Cache>,
) -> Option<(String, Option<Vec<u8>>)> {
    for attempt in 0..ATTEMPTS {
        let key = format!("key-{attempt}");
        let started = Instant::now();
        cache.set(&key, b"value".to_vec(), SHORT).await.unwrap();
        let read = cache.get(&key).await.unwrap();
        if started.elapsed() < SHORT {
            return Some((key, read));
        }
    }
    None
}

#[tokio::test]
async fn a_value_is_gone_after_its_ttl() {
    let cache: Arc<dyn Cache> = Arc::new(MokaCache::new());

    let (key, before) = stored_and_read_back_within_the_ttl(&cache)
        .await
        .expect("no read was made inside the TTL in 100 attempts");
    tokio::time::sleep(PAST_SHORT).await;
    let after = cache.get(&key).await.unwrap();

    assert_eq!(before, Some(b"value".to_vec()));
    assert_eq!(after, None);
}

#[tokio::test]
async fn overwriting_a_key_replaces_its_ttl() {
    let cache: Arc<dyn Cache> = Arc::new(MokaCache::new());

    cache.set("shortened", b"v".to_vec(), LONG).await.unwrap();
    cache.set("shortened", b"v".to_vec(), SHORT).await.unwrap();
    cache.set("lengthened", b"v".to_vec(), SHORT).await.unwrap();
    cache.set("lengthened", b"v".to_vec(), LONG).await.unwrap();
    tokio::time::sleep(PAST_SHORT).await;

    assert_eq!(cache.get("shortened").await.unwrap(), None);
    assert_eq!(cache.get("lengthened").await.unwrap(), Some(b"v".to_vec()));
}
