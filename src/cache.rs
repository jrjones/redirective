//! cache module: maintains thread-safe router cache.

use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;

/// Thread-safe cache for URL redirects.
#[derive(Clone)]
pub struct RouterCache {
    inner: Arc<ArcSwap<HashMap<String, String>>>,
}

impl RouterCache {
    /// Create a new RouterCache with initial mappings.
    pub fn new(initial: HashMap<String, String>) -> Self {
        let swap = ArcSwap::new(Arc::new(initial));
        RouterCache { inner: Arc::new(swap) }
    }

    /// Lookup a code in the cache, returning the target URL if found.
    pub fn lookup(&self, code: &str) -> Option<String> {
        let map_arc = self.inner.load();
        map_arc.get(code).cloned()
    }

    /// Atomically swap in a new mapping.
    pub fn swap(&self, new_map: HashMap<String, String>) {
        self.inner.store(Arc::new(new_map));
    }
}

#[cfg(test)]
mod tests {
    use super::RouterCache;
    use std::collections::HashMap;

    #[test]
    fn lookup_existing() {
        let mut m = HashMap::new();
        m.insert("a".to_string(), "1".to_string());
        let cache = RouterCache::new(m);
        assert_eq!(cache.lookup("a"), Some("1".to_string()));
    }

    #[test]
    fn lookup_missing() {
        let m = HashMap::new();
        let cache = RouterCache::new(m);
        assert_eq!(cache.lookup("missing"), None);
    }

    #[test]
    fn swap_updates() {
        let mut m1 = HashMap::new();
        m1.insert("a".to_string(), "1".to_string());
        let cache = RouterCache::new(m1);
        assert_eq!(cache.lookup("a"), Some("1".to_string()));
        let mut m2 = HashMap::new();
        m2.insert("b".to_string(), "2".to_string());
        cache.swap(m2);
        assert_eq!(cache.lookup("a"), None);
        assert_eq!(cache.lookup("b"), Some("2".to_string()));
    }
}