//! Unit tests for route resolution cache
//!
//! Tests for src/cache.rs - verifying insertion, eviction,
//! hit/miss, and explicit removal as specified in T029.

#[cfg(test)]
mod cache_tests {
    use gpui_navigator::cache::{RouteCache, RouteId};

    #[test]
    fn test_cache_insertion() {
        let mut cache = RouteCache::new();

        // Initially empty
        assert_eq!(cache.parent_cache_size(), 0);

        // Insert parent route mapping
        let parent_id = RouteId::from_path("/dashboard");
        cache.set_parent("/dashboard/analytics".to_string(), parent_id);

        // Should have one entry
        assert_eq!(cache.parent_cache_size(), 1);

        // Should be able to retrieve it
        let retrieved = cache.get_parent("/dashboard/analytics");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().path, "/dashboard");
    }

    #[test]
    fn test_cache_eviction() {
        // Create cache with small capacity to test eviction
        let mut cache = RouteCache::with_capacity(3);

        // Insert 3 entries (at capacity)
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));
        cache.set_parent("/c".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 3);

        // Access /b to make it more recent than /a
        cache.get_parent("/b");

        // Insert 4th entry - should evict /a (least recently used)
        cache.set_parent("/d".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 3);

        // /a should be evicted (miss)
        let result_a = cache.get_parent("/a");
        assert!(result_a.is_none());

        // /b should still exist (was accessed more recently)
        let result_b = cache.get_parent("/b");
        assert!(result_b.is_some());

        // /d should exist (just inserted)
        let result_d = cache.get_parent("/d");
        assert!(result_d.is_some());
    }

    #[test]
    fn test_cache_hit_miss() {
        let mut cache = RouteCache::new();

        // Miss - entry doesn't exist
        let result = cache.get_parent("/nonexistent");
        assert!(result.is_none());
        assert_eq!(cache.stats().parent_misses, 1);
        assert_eq!(cache.stats().parent_hits, 0);

        // Insert entry
        cache.set_parent(
            "/dashboard/analytics".to_string(),
            RouteId::from_path("/dashboard"),
        );

        // Hit - entry exists
        let result = cache.get_parent("/dashboard/analytics");
        assert!(result.is_some());
        assert_eq!(cache.stats().parent_hits, 1);
        assert_eq!(cache.stats().parent_misses, 1);

        // Another hit - accessing same entry
        let result = cache.get_parent("/dashboard/analytics");
        assert!(result.is_some());
        assert_eq!(cache.stats().parent_hits, 2);
        assert_eq!(cache.stats().parent_misses, 1);
    }

    #[test]
    fn test_explicit_removal() {
        let mut cache = RouteCache::new();

        // Insert multiple entries
        cache.set_parent(
            "/dashboard/analytics".to_string(),
            RouteId::from_path("/dashboard"),
        );
        cache.set_parent(
            "/dashboard/settings".to_string(),
            RouteId::from_path("/dashboard"),
        );
        cache.set_parent(
            "/products/list".to_string(),
            RouteId::from_path("/products"),
        );
        assert_eq!(cache.parent_cache_size(), 3);

        // Clear entire cache
        cache.clear();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.stats().invalidations, 1);

        // All entries should be gone
        assert!(cache.get_parent("/dashboard/analytics").is_none());
        assert!(cache.get_parent("/dashboard/settings").is_none());
        assert!(cache.get_parent("/products/list").is_none());
    }

    #[test]
    fn test_cache_stats_tracking() {
        let mut cache = RouteCache::new();

        // Track misses
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/c");
        assert_eq!(cache.stats().parent_misses, 3);
        assert_eq!(cache.stats().parent_hits, 0);

        // Insert some entries
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));

        // Track hits
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/a");
        assert_eq!(cache.stats().parent_hits, 3);
        assert_eq!(cache.stats().parent_misses, 3);

        // Test hit rate calculation
        let hit_rate = cache.stats().parent_hit_rate();
        assert!((hit_rate - 0.5).abs() < 0.001); // 3 hits / 6 total = 0.5
    }

    #[test]
    fn test_cache_stats_reset() {
        let mut cache = RouteCache::new();

        // Generate some stats
        cache.get_parent("/a");
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.get_parent("/a");
        assert_eq!(cache.stats().parent_hits, 1);
        assert_eq!(cache.stats().parent_misses, 1);

        // Reset stats
        cache.reset_stats();
        assert_eq!(cache.stats().parent_hits, 0);
        assert_eq!(cache.stats().parent_misses, 0);

        // Cache entries should still exist
        let result = cache.get_parent("/a");
        assert!(result.is_some());
    }

    #[test]
    fn test_lru_ordering_with_access() {
        // Create small cache to test LRU behavior
        let mut cache = RouteCache::with_capacity(2);

        // Insert two entries
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 2);

        // Access /a to make it more recent
        cache.get_parent("/a");

        // Insert new entry - should evict /b (least recent)
        cache.set_parent("/c".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 2);

        // /a should still exist (was accessed)
        assert!(cache.get_parent("/a").is_some());

        // /b should be evicted
        assert!(cache.get_parent("/b").is_none());

        // /c should exist (just inserted)
        assert!(cache.get_parent("/c").is_some());
    }

    #[test]
    fn test_cache_capacity() {
        let capacity = 5;
        let mut cache = RouteCache::with_capacity(capacity);

        // Fill to capacity
        for i in 0..capacity {
            cache.set_parent(format!("/route{}", i), RouteId::from_path("/"));
        }
        assert_eq!(cache.parent_cache_size(), capacity);

        // Inserting more should maintain capacity
        cache.set_parent("/extra1".to_string(), RouteId::from_path("/"));
        cache.set_parent("/extra2".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), capacity);
    }

    #[test]
    fn test_overall_hit_rate() {
        let mut cache = RouteCache::new();

        // 2 misses
        cache.get_parent("/a");
        cache.get_parent("/b");

        // Insert
        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));

        // 3 hits
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/a");

        // Overall: 3 hits / 5 total = 0.6
        let overall_rate = cache.stats().overall_hit_rate();
        assert!((overall_rate - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_route_id_creation() {
        // Test from_path
        let id1 = RouteId::from_path("/dashboard");
        assert_eq!(id1.path, "/dashboard");

        // Test from_path with String
        let id2 = RouteId::from_path("/dashboard/analytics".to_string());
        assert_eq!(id2.path, "/dashboard/analytics");

        // Test equality
        let id3 = RouteId::from_path("/dashboard");
        assert_eq!(id1, id3);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_cache_default() {
        let cache = RouteCache::default();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.child_cache_size(), 0);
    }

    #[test]
    fn test_total_size() {
        let mut cache = RouteCache::new();
        assert_eq!(cache.total_size(), 0);

        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.total_size(), 1);
        assert_eq!(cache.parent_cache_size(), 1);
        assert_eq!(cache.child_cache_size(), 0);
    }
}
