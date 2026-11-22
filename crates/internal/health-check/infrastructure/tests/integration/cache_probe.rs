use std::sync::Arc;

use health_check_domain::{Dependency, HealthProbe as _};
use health_check_infrastructure::CacheProbe;
use shared_infrastructure::cache::MokaCache;
use test_support::cache::StubCache;

/// How many probes run at once against one cache, as concurrent health requests do.
const CONCURRENT_PROBES: usize = 32;

#[tokio::test]
async fn the_in_process_cache_of_the_service_passes_every_time() {
    let probe = CacheProbe::new(Arc::new(MokaCache::new()));

    for _ in 0..3 {
        assert!(probe.check().await.is_ok());
    }
    assert_eq!(probe.dependency(), Dependency::Cache);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_probes_over_one_cache_all_pass() {
    let probe = CacheProbe::new(Arc::new(MokaCache::new()));

    let tasks: Vec<_> = (0..CONCURRENT_PROBES)
        .map(|_| {
            let probe = probe.clone();
            tokio::spawn(async move { probe.check().await.map_err(|error| error.to_string()) })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.expect("a probe task does not panic"));
    }

    assert_eq!(results, vec![Ok(()); CONCURRENT_PROBES]);
}

#[tokio::test]
async fn a_failing_cache_fails_the_probe() {
    let probe = CacheProbe::new(Arc::new(StubCache::failing()));

    let result = probe.check().await;

    assert_eq!(result.unwrap_err().to_string(), "cache operation failed");
}
