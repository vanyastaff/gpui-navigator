//! Route resolution caching.
//!
//! This module provides [`RouteCache`] — an LRU-based cache that avoids
//! repeated route tree lookups during rendering. It is gated behind the
//! `cache` feature flag and uses the [`lru`] crate internally.
//!
//! Two independent LRU caches are maintained:
//!
//! - **Parent cache** — maps a full request path to the [`RouteId`] of the
//!   parent route that owns it (e.g. `"/dashboard/analytics"` → `"/dashboard"`).
//! - **Child cache** — maps an `(path, outlet_name)` pair to resolved
//!   [`RouteParams`].
//!
//! [`CacheStats`] tracks hits, misses, and invalidations so you can monitor
//! cache effectiveness at runtime.
//!
//! # Examples
//!
//! ```
//! use gpui_navigator::cache::{RouteCache, RouteId};
//!
//! let mut cache = RouteCache::new();
//! cache.set_parent("/dashboard/analytics".to_string(), RouteId::from_path("/dashboard"));
//!
//! assert_eq!(cache.get_parent("/dashboard/analytics").unwrap().path, "/dashboard");
//! assert_eq!(cache.stats().parent_hits, 1);
//! ```

use crate::route::Route;
use crate::{debug_log, trace_log, RouteParams};
use lru::LruCache;
use std::num::NonZeroUsize;

/// Unique identifier for a route in the tree
///
/// This allows us to reference routes without storing full Route clones.
/// Routes are identified by their path hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteId {
    /// Full path of the route (e.g., "/dashboard/analytics")
    pub path: String,
}

impl RouteId {
    /// Create a new route ID from a route
    pub fn from_route(route: &Route) -> Self {
        Self {
            path: route.config.path.clone(),
        }
    }

    /// Create a route ID from a path string
    pub fn from_path(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

/// Cache key for outlet resolution
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OutletCacheKey {
    path: String,
    outlet_name: Option<String>,
}

/// Cached result of finding a parent route
#[derive(Debug, Clone)]
struct ParentRouteCacheEntry {
    parent_route_id: RouteId,
}

/// Counters tracking cache hit/miss rates and invalidations.
///
/// Use [`parent_hit_rate`](Self::parent_hit_rate),
/// [`child_hit_rate`](Self::child_hit_rate), or
/// [`overall_hit_rate`](Self::overall_hit_rate) for quick ratio access.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of parent-cache hits.
    pub parent_hits: usize,
    /// Number of parent-cache misses.
    pub parent_misses: usize,
    /// Number of child-cache hits.
    pub child_hits: usize,
    /// Number of child-cache misses.
    pub child_misses: usize,
    /// Number of full cache invalidations (via [`RouteCache::clear`]).
    pub invalidations: usize,
}

impl CacheStats {
    /// Return the parent-cache hit rate as a value in `0.0..=1.0`.
    ///
    /// Returns `0.0` if no parent lookups have been performed.
    #[allow(clippy::cast_precision_loss)]
    pub fn parent_hit_rate(&self) -> f64 {
        let total = self.parent_hits + self.parent_misses;
        if total == 0 {
            0.0
        } else {
            self.parent_hits as f64 / total as f64
        }
    }

    /// Return the child-cache hit rate as a value in `0.0..=1.0`.
    #[allow(clippy::cast_precision_loss)]
    pub fn child_hit_rate(&self) -> f64 {
        let total = self.child_hits + self.child_misses;
        if total == 0 {
            0.0
        } else {
            self.child_hits as f64 / total as f64
        }
    }

    /// Return the combined (parent + child) hit rate as a value in `0.0..=1.0`.
    #[allow(clippy::cast_precision_loss)]
    pub fn overall_hit_rate(&self) -> f64 {
        let total_hits = self.parent_hits + self.child_hits;
        let total_misses = self.parent_misses + self.child_misses;
        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }
}

/// LRU cache for route resolution results.
///
/// Maintains separate parent and child caches, each with independent LRU
/// eviction. Default capacity is 1000 entries per cache.
///
/// The cache is automatically cleared on route registration and navigation
/// to ensure consistency.
#[derive(Debug)]
pub struct RouteCache {
    parent_cache: LruCache<String, ParentRouteCacheEntry>,
    child_cache: LruCache<OutletCacheKey, RouteParams>,
    stats: CacheStats,
}

impl RouteCache {
    const DEFAULT_CAPACITY: usize = 1000;

    /// Create a cache with the default capacity (1000 entries per sub-cache).
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// Create a cache with a custom per-sub-cache capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("Cache capacity must be non-zero");
        Self {
            parent_cache: LruCache::new(cap),
            child_cache: LruCache::new(cap),
            stats: CacheStats::default(),
        }
    }

    /// Clear both sub-caches and increment the invalidation counter.
    pub fn clear(&mut self) {
        let parent_len = self.parent_cache.len();
        let child_len = self.child_cache.len();
        self.parent_cache.clear();
        self.child_cache.clear();
        self.stats.invalidations += 1;
        debug_log!(
            "Cache cleared: {} parent + {} child entries removed ({} total invalidations, parent hit rate: {:.1}%)",
            parent_len,
            child_len,
            self.stats.invalidations,
            self.stats.parent_hit_rate() * 100.0
        );
    }

    /// Look up the cached parent [`RouteId`] for the given `path`.
    ///
    /// Returns `None` on a cache miss. Updates hit/miss stats.
    pub fn get_parent(&mut self, path: &str) -> Option<RouteId> {
        if let Some(entry) = self.parent_cache.get(path) {
            self.stats.parent_hits += 1;
            trace_log!("Parent cache hit for path: '{}'", path);
            Some(entry.parent_route_id.clone())
        } else {
            self.stats.parent_misses += 1;
            trace_log!("Parent cache miss for path: '{}'", path);
            None
        }
    }

    /// Insert a parent route mapping into the cache.
    pub fn set_parent(&mut self, path: String, parent_route_id: RouteId) {
        trace_log!(
            "Caching parent route '{}' for path '{}'",
            parent_route_id.path,
            path
        );
        self.parent_cache
            .push(path, ParentRouteCacheEntry { parent_route_id });
    }

    /// Return a reference to the current cache statistics.
    pub const fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Reset all counters in [`CacheStats`] to zero.
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    /// Return the number of entries currently in the parent cache.
    pub fn parent_cache_size(&self) -> usize {
        self.parent_cache.len()
    }

    /// Return the number of entries currently in the child cache.
    pub fn child_cache_size(&self) -> usize {
        self.child_cache.len()
    }

    /// Return the total number of entries across both sub-caches.
    pub fn total_size(&self) -> usize {
        self.parent_cache_size() + self.child_cache_size()
    }
}

impl Default for RouteCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RouteCache {
    fn clone(&self) -> Self {
        let parent_cap = self.parent_cache.cap();
        let child_cap = self.child_cache.cap();
        Self {
            parent_cache: LruCache::new(parent_cap),
            child_cache: LruCache::new(child_cap),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = RouteCache::new();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.stats().parent_hits, 0);
    }

    #[test]
    fn test_parent_cache_miss() {
        let mut cache = RouteCache::new();
        let result = cache.get_parent("/dashboard");
        assert!(result.is_none());
        assert_eq!(cache.stats().parent_misses, 1);
    }

    #[test]
    fn test_parent_cache_hit() {
        let mut cache = RouteCache::new();
        let route_id = RouteId::from_path("/dashboard");
        cache.set_parent("/dashboard/analytics".to_string(), route_id);

        let result = cache.get_parent("/dashboard/analytics");
        assert!(result.is_some());
        assert_eq!(result.unwrap().path, "/dashboard");
        assert_eq!(cache.stats().parent_hits, 1);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = RouteCache::new();
        cache.set_parent("/dashboard".to_string(), RouteId::from_path("/"));
        assert_eq!(cache.parent_cache_size(), 1);

        cache.clear();
        assert_eq!(cache.parent_cache_size(), 0);
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut cache = RouteCache::new();
        cache.get_parent("/a");
        cache.get_parent("/b");
        cache.get_parent("/c");

        cache.set_parent("/a".to_string(), RouteId::from_path("/"));
        cache.set_parent("/b".to_string(), RouteId::from_path("/"));

        cache.get_parent("/a");
        cache.get_parent("/b");

        assert_eq!(cache.stats().parent_hits, 2);
        assert_eq!(cache.stats().parent_misses, 3);
        assert!((cache.stats().parent_hit_rate() - 0.4).abs() < 0.001);
    }
}
