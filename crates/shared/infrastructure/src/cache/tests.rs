use std::sync::Arc;
use std::time::Duration;

use shared_domain::Cache;

use super::MokaCache;

const LONG: Duration = Duration::from_secs(3600);

#[tokio::test]
async fn reads_back_what_was_set() {
    let cache: Arc<dyn Cache> = Arc::new(MokaCache::new());

    cache.set("key", b"value".to_vec(), LONG).await.unwrap();

    assert_eq!(cache.get("key").await.unwrap(), Some(b"value".to_vec()));
}

#[tokio::test]
async fn reads_nothing_for_an_unknown_key() {
    let cache = MokaCache::default();

    assert_eq!(cache.get("missing").await.unwrap(), None);
}

#[tokio::test]
async fn overwrites_the_value_of_a_key() {
    let cache = MokaCache::new();

    cache.set("key", b"one".to_vec(), LONG).await.unwrap();
    cache.set("key", b"two".to_vec(), LONG).await.unwrap();

    assert_eq!(cache.get("key").await.unwrap(), Some(b"two".to_vec()));
}

#[tokio::test]
async fn debug_names_the_type_and_counts_the_entries_without_printing_values() {
    let cache = MokaCache::new();
    assert_eq!(format!("{cache:?}"), "MokaCache { entries: 0 }");

    cache
        .set("secret-key", b"secret-value".to_vec(), LONG)
        .await
        .unwrap();
    cache.entries.run_pending_tasks().await;
    let text = format!("{cache:?}");

    assert_eq!(text, "MokaCache { entries: 1 }");
    assert!(!text.contains("secret"));
}
