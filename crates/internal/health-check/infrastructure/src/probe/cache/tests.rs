use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use health_check_domain::{Dependency, HealthProbe as _};
use shared_domain::{Cache, CacheError};
use test_support::cache::StubCache;

use super::CacheProbe;

type Writes = Vec<(String, Vec<u8>, Duration)>;

/// Records every write and every read key, then delegates to a stub.
#[derive(Debug)]
struct RecordingCache {
    inner: StubCache,
    writes: Mutex<Writes>,
    reads: Mutex<Vec<String>>,
}

impl RecordingCache {
    fn over(inner: StubCache) -> Arc<Self> {
        Arc::new(Self {
            inner,
            writes: Mutex::new(Vec::new()),
            reads: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait]
impl Cache for RecordingCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        self.reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(key.to_owned());
        self.inner.get(key).await
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), CacheError> {
        self.writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((key.to_owned(), value.clone(), ttl));
        self.inner.set(key, value, ttl).await
    }
}

async fn run(stub: StubCache) -> (Result<(), String>, Arc<RecordingCache>) {
    let cache = RecordingCache::over(stub);
    let probe = CacheProbe::new(Arc::<RecordingCache>::clone(&cache));
    let result = probe.check().await.map_err(|error| error.to_string());
    (result, cache)
}

#[tokio::test]
async fn a_working_cache_passes() {
    let (result, _) = run(StubCache::working()).await;

    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn a_failing_cache_fails_on_the_write() {
    let (result, cache) = run(StubCache::failing()).await;

    assert_eq!(result, Err("cache operation failed".to_owned()));
    assert_eq!(
        cache
            .reads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len(),
        0
    );
}

#[tokio::test]
async fn a_cache_that_forgets_the_value_fails() {
    let (result, _) = run(StubCache::forgetful()).await;

    assert_eq!(
        result,
        Err("the cache did not return the value that was stored".to_owned())
    );
}

#[tokio::test]
async fn a_cache_that_corrupts_the_value_fails() {
    let (result, _) = run(StubCache::corrupting()).await;

    assert_eq!(
        result,
        Err("the cache returned a different value than the one stored".to_owned())
    );
}

#[tokio::test]
async fn a_read_error_fails() {
    #[derive(Debug)]
    struct WriteOnly;

    #[async_trait::async_trait]
    impl Cache for WriteOnly {
        async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, CacheError> {
            Err(CacheError("read failed".into()))
        }

        async fn set(&self, _key: &str, _value: Vec<u8>, _ttl: Duration) -> Result<(), CacheError> {
            Ok(())
        }
    }
    let probe = CacheProbe::new(Arc::new(WriteOnly));

    let result = probe.check().await;

    assert_eq!(result.unwrap_err().to_string(), "cache operation failed");
}

#[tokio::test]
async fn every_call_writes_the_key_with_ok_and_a_minute_then_reads_it() {
    let (_, cache) = run(StubCache::working()).await;
    let probe = CacheProbe::new(Arc::<RecordingCache>::clone(&cache));
    probe.check().await.unwrap();

    let expected = (
        "health_check".to_owned(),
        b"ok".to_vec(),
        Duration::from_secs(60),
    );
    assert_eq!(
        *cache.writes.lock().unwrap_or_else(PoisonError::into_inner),
        vec![expected.clone(), expected]
    );
    assert_eq!(
        *cache.reads.lock().unwrap_or_else(PoisonError::into_inner),
        vec!["health_check".to_owned(); 2]
    );
}

#[test]
fn the_probe_stands_for_the_cache() {
    let probe = CacheProbe::new(Arc::new(StubCache::working()));

    assert_eq!(probe.dependency(), Dependency::Cache);
}
