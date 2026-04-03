use std::collections::HashMap;

/// Default maximum number of cache entries.
const DEFAULT_MAX_ENTRIES: usize = 100;

/// Default maximum total cache size in bytes (25 MB).
const DEFAULT_MAX_SIZE: usize = 25 * 1024 * 1024;

/// A cached file entry with content and access metadata.
#[derive(Debug, Clone)]
struct CacheEntry {
    content: String,
    size: usize,
    last_access: u64,
}

/// A simple LRU file cache for caching file contents in memory.
///
/// Evicts least-recently-used entries when max_entries or max_size is exceeded.
pub struct FileStateCache {
    entries: HashMap<String, CacheEntry>,
    max_entries: usize,
    max_size: usize,
    current_size: usize,
    access_counter: u64,
}

impl FileStateCache {
    /// Create a new file cache with default limits (100 entries, 25 MB).
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
            max_size: DEFAULT_MAX_SIZE,
            current_size: 0,
            access_counter: 0,
        }
    }

    /// Create a new file cache with custom limits.
    pub fn with_limits(max_entries: usize, max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            max_size,
            current_size: 0,
            access_counter: 0,
        }
    }

    /// Get a cached file's content by path. Returns None if not cached.
    pub fn get(&mut self, path: &str) -> Option<&str> {
        self.access_counter += 1;
        let counter = self.access_counter;
        if let Some(entry) = self.entries.get_mut(path) {
            entry.last_access = counter;
            Some(&entry.content)
        } else {
            None
        }
    }

    /// Cache file content for a given path. Evicts LRU entries if limits are exceeded.
    pub fn set(&mut self, path: String, content: String) {
        let size = content.len();

        // Remove existing entry for same path first
        if let Some(old) = self.entries.remove(&path) {
            self.current_size -= old.size;
        }

        // Evict until we have room
        while self.entries.len() >= self.max_entries
            || (self.current_size + size > self.max_size && !self.entries.is_empty())
        {
            self.evict_lru();
        }

        // If a single entry exceeds max_size, don't cache it
        if size > self.max_size {
            return;
        }

        self.access_counter += 1;
        self.entries.insert(
            path,
            CacheEntry {
                content,
                size,
                last_access: self.access_counter,
            },
        );
        self.current_size += size;
    }

    /// Remove a cached entry by path.
    pub fn delete(&mut self, path: &str) -> bool {
        if let Some(entry) = self.entries.remove(path) {
            self.current_size -= entry.size;
            true
        } else {
            false
        }
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
        self.access_counter = 0;
    }

    /// Number of entries currently cached.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total size of cached content in bytes.
    pub fn total_size(&self) -> usize {
        self.current_size
    }

    /// Evict the least recently used entry.
    fn evict_lru(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        let lru_key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone());

        if let Some(key) = lru_key {
            if let Some(entry) = self.entries.remove(&key) {
                self.current_size -= entry.size;
            }
        }
    }
}

impl Default for FileStateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_get_set() {
        let mut cache = FileStateCache::new();
        cache.set("/foo/bar.rs".to_string(), "hello world".to_string());
        assert_eq!(cache.get("/foo/bar.rs"), Some("hello world"));
        assert_eq!(cache.get("/nonexistent"), None);
    }

    #[test]
    fn test_delete() {
        let mut cache = FileStateCache::new();
        cache.set("/a.rs".to_string(), "content".to_string());
        assert!(cache.delete("/a.rs"));
        assert!(!cache.delete("/a.rs"));
        assert_eq!(cache.get("/a.rs"), None);
    }

    #[test]
    fn test_clear() {
        let mut cache = FileStateCache::new();
        cache.set("/a.rs".to_string(), "a".to_string());
        cache.set("/b.rs".to_string(), "b".to_string());
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.total_size(), 0);
    }

    #[test]
    fn test_lru_eviction_by_entries() {
        let mut cache = FileStateCache::with_limits(2, 1024 * 1024);
        cache.set("/a.rs".to_string(), "aaa".to_string());
        cache.set("/b.rs".to_string(), "bbb".to_string());
        // Access /a.rs to make it more recent
        cache.get("/a.rs");
        // Adding a third entry should evict /b.rs (least recently used)
        cache.set("/c.rs".to_string(), "ccc".to_string());
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("/b.rs"), None);
        assert!(cache.get("/a.rs").is_some());
        assert!(cache.get("/c.rs").is_some());
    }

    #[test]
    fn test_lru_eviction_by_size() {
        let mut cache = FileStateCache::with_limits(100, 10);
        cache.set("a".to_string(), "12345".to_string()); // 5 bytes
        cache.set("b".to_string(), "12345".to_string()); // 5 bytes, total = 10
        cache.set("c".to_string(), "12345".to_string()); // would exceed 10, evicts LRU
        assert_eq!(cache.len(), 2);
        assert!(cache.total_size() <= 10);
    }

    #[test]
    fn test_oversize_entry_rejected() {
        let mut cache = FileStateCache::with_limits(100, 5);
        cache.set("big".to_string(), "123456".to_string()); // 6 bytes > max 5
        assert!(cache.is_empty());
    }

    #[test]
    fn test_update_existing_key() {
        let mut cache = FileStateCache::new();
        cache.set("f".to_string(), "old".to_string());
        cache.set("f".to_string(), "new content".to_string());
        assert_eq!(cache.get("f"), Some("new content"));
        assert_eq!(cache.len(), 1);
    }
}
