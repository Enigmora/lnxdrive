//! Inode table for bidirectional inode ↔ item_id mapping.
//!
//! Provides lock-free concurrent access for FUSE operations.

use std::sync::Arc;

use dashmap::DashMap;
use lnxdrive_core::domain::newtypes::UniqueId;

use crate::inode_entry::InodeEntry;

/// Bidirectional mapping between inodes and items.
///
/// Uses DashMap for lock-free concurrent access from multiple FUSE threads.
pub struct InodeTable {
    /// inode -> entry mapping
    by_inode: DashMap<u64, Arc<InodeEntry>>,
    /// item_id -> inode mapping (reverse lookup)
    by_item_id: DashMap<UniqueId, u64>,
}

impl InodeTable {
    /// Create a new empty inode table.
    pub fn new() -> Self {
        Self {
            by_inode: DashMap::new(),
            by_item_id: DashMap::new(),
        }
    }

    /// Insert a new inode entry into the table.
    ///
    /// Creates bidirectional mapping between inode number and item_id.
    pub fn insert(&self, entry: InodeEntry) {
        let ino = entry.ino().get();
        let item_id = *entry.item_id();
        let entry = Arc::new(entry);
        self.by_inode.insert(ino, entry);
        self.by_item_id.insert(item_id, ino);
    }

    /// Retrieve an inode entry by its inode number.
    pub fn get(&self, ino: u64) -> Option<Arc<InodeEntry>> {
        self.by_inode.get(&ino).map(|r| Arc::clone(&r))
    }

    /// Retrieve an inode number by its item_id.
    pub fn get_by_item_id(&self, id: &UniqueId) -> Option<u64> {
        self.by_item_id.get(id).map(|r| *r)
    }

    /// Remove an inode entry by its inode number.
    ///
    /// Removes both the inode->entry and item_id->inode mappings.
    pub fn remove(&self, ino: u64) -> Option<Arc<InodeEntry>> {
        if let Some((_, entry)) = self.by_inode.remove(&ino) {
            self.by_item_id.remove(entry.item_id());
            Some(entry)
        } else {
            None
        }
    }

    /// Retrieve all child entries of a parent inode.
    ///
    /// Returns a vector of all entries whose parent_ino matches the given value.
    pub fn children(&self, parent_ino: u64) -> Vec<Arc<InodeEntry>> {
        self.by_inode
            .iter()
            .filter(|r| r.value().parent_ino().get() == parent_ino)
            .map(|r| Arc::clone(r.value()))
            .collect()
    }

    /// Look up a child entry by parent inode and name.
    ///
    /// Performs a linear search through entries to find a matching child.
    pub fn lookup(&self, parent_ino: u64, name: &str) -> Option<Arc<InodeEntry>> {
        self.by_inode
            .iter()
            .find(|r| r.value().parent_ino().get() == parent_ino && r.value().name() == name)
            .map(|r| Arc::clone(r.value()))
    }

    /// Get the total number of entries in the table.
    pub fn len(&self) -> usize {
        self.by_inode.len()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.by_inode.is_empty()
    }
}

impl Default for InodeTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use lnxdrive_core::domain::{ItemState, RemoteId};

    use super::*;
    use crate::inode_entry::InodeNumber;

    /// Helper function to create a test InodeEntry with minimal required fields.
    fn make_test_entry(ino: u64, parent_ino: u64, name: &str, is_dir: bool) -> InodeEntry {
        InodeEntry::new(
            InodeNumber::new(ino),
            UniqueId::new(),
            Some(RemoteId::new(format!("remote_{}", ino)).unwrap()),
            InodeNumber::new(parent_ino),
            name.to_string(),
            if is_dir {
                fuser::FileType::Directory
            } else {
                fuser::FileType::RegularFile
            },
            1024,                               // size
            if is_dir { 0o755 } else { 0o644 }, // perm
            SystemTime::now(),
            SystemTime::now(),
            SystemTime::now(),
            1, // nlink
            ItemState::Online,
        )
    }

    /// Helper function to create a test InodeEntry with a specific item_id.
    fn make_entry_with_id(ino: u64, parent_ino: u64, name: &str, item_id: UniqueId) -> InodeEntry {
        InodeEntry::new(
            InodeNumber::new(ino),
            item_id,
            Some(RemoteId::new(format!("remote_{}", ino)).unwrap()),
            InodeNumber::new(parent_ino),
            name.to_string(),
            fuser::FileType::RegularFile,
            1024,
            0o644,
            SystemTime::now(),
            SystemTime::now(),
            SystemTime::now(),
            1,
            ItemState::Online,
        )
    }

    #[test]
    fn test_insert_and_get() {
        let table = InodeTable::new();
        let entry = make_test_entry(42, 1, "test.txt", false);
        let item_id = *entry.item_id();

        table.insert(entry);

        // Test get by inode
        let retrieved = table.get(42).expect("Entry should exist");
        assert_eq!(retrieved.ino().get(), 42);
        assert_eq!(retrieved.name(), "test.txt");
        assert_eq!(*retrieved.item_id(), item_id);

        // Test get non-existent inode
        assert!(table.get(999).is_none());
    }

    #[test]
    fn test_get_by_item_id() {
        let table = InodeTable::new();
        let item_id = UniqueId::new();
        let entry = make_entry_with_id(42, 1, "test.txt", item_id);

        table.insert(entry);

        // Test reverse lookup
        let ino = table.get_by_item_id(&item_id).expect("Item should exist");
        assert_eq!(ino, 42);

        // Test get non-existent item_id
        let random_id = UniqueId::new();
        assert!(table.get_by_item_id(&random_id).is_none());
    }

    #[test]
    fn test_remove() {
        let table = InodeTable::new();
        let item_id = UniqueId::new();
        let entry = make_entry_with_id(42, 1, "test.txt", item_id);

        table.insert(entry);

        // Verify entry exists
        assert!(table.get(42).is_some());
        assert!(table.get_by_item_id(&item_id).is_some());

        // Remove entry
        let removed = table.remove(42).expect("Entry should be removed");
        assert_eq!(removed.ino().get(), 42);

        // Verify both mappings are removed
        assert!(table.get(42).is_none());
        assert!(table.get_by_item_id(&item_id).is_none());

        // Remove non-existent entry
        assert!(table.remove(999).is_none());
    }

    #[test]
    fn test_children() {
        let table = InodeTable::new();

        // Create parent directory (inode 10)
        table.insert(make_test_entry(10, 1, "parent", true));

        // Create children of parent (inode 10)
        table.insert(make_test_entry(20, 10, "child1.txt", false));
        table.insert(make_test_entry(21, 10, "child2.txt", false));
        table.insert(make_test_entry(22, 10, "subdir", true));

        // Create entry in different parent
        table.insert(make_test_entry(30, 1, "other.txt", false));

        // Get children of parent
        let children = table.children(10);

        assert_eq!(children.len(), 3);
        let names: Vec<String> = children.iter().map(|e| e.name().to_string()).collect();
        assert!(names.contains(&"child1.txt".to_string()));
        assert!(names.contains(&"child2.txt".to_string()));
        assert!(names.contains(&"subdir".to_string()));
        assert!(!names.contains(&"other.txt".to_string()));

        // Test non-existent parent
        let no_children = table.children(999);
        assert_eq!(no_children.len(), 0);
    }

    #[test]
    fn test_lookup() {
        let table = InodeTable::new();

        // Create parent directory
        table.insert(make_test_entry(10, 1, "parent", true));

        // Create children
        table.insert(make_test_entry(20, 10, "file1.txt", false));
        table.insert(make_test_entry(21, 10, "file2.txt", false));

        // Test successful lookup
        let found = table.lookup(10, "file1.txt").expect("Should find entry");
        assert_eq!(found.ino().get(), 20);
        assert_eq!(found.name(), "file1.txt");

        // Test lookup with wrong parent
        assert!(table.lookup(999, "file1.txt").is_none());

        // Test lookup with wrong name
        assert!(table.lookup(10, "nonexistent.txt").is_none());

        // Test lookup with both wrong
        assert!(table.lookup(999, "nonexistent.txt").is_none());
    }

    #[test]
    fn test_len_and_is_empty() {
        let table = InodeTable::new();

        assert!(table.is_empty());
        assert_eq!(table.len(), 0);

        table.insert(make_test_entry(10, 1, "file1.txt", false));
        assert!(!table.is_empty());
        assert_eq!(table.len(), 1);

        table.insert(make_test_entry(11, 1, "file2.txt", false));
        assert_eq!(table.len(), 2);

        table.remove(10);
        assert_eq!(table.len(), 1);

        table.remove(11);
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_concurrent_access_multiple_threads() {
        use std::{sync::Arc, thread};

        let table = Arc::new(InodeTable::new());
        let num_threads = 10;
        let entries_per_thread = 100;

        // Spawn threads that concurrently insert entries
        let mut handles = vec![];
        for thread_id in 0..num_threads {
            let table_clone = Arc::clone(&table);
            let handle = thread::spawn(move || {
                for i in 0..entries_per_thread {
                    let ino = (thread_id * entries_per_thread + i) as u64 + 1000;
                    let entry = make_test_entry(ino, 1, &format!("file_{}.txt", ino), false);
                    table_clone.insert(entry);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete");
        }

        // Verify all entries were inserted
        let expected_count = num_threads * entries_per_thread;
        assert_eq!(table.len(), expected_count);

        // Verify we can retrieve entries
        for thread_id in 0..num_threads {
            for i in 0..entries_per_thread {
                let ino = (thread_id * entries_per_thread + i) as u64 + 1000;
                assert!(table.get(ino).is_some(), "Entry {} should exist", ino);
            }
        }
    }

    #[test]
    fn test_concurrent_insert_and_remove() {
        use std::{sync::Arc, thread};

        let table = Arc::new(InodeTable::new());

        // Pre-populate table
        for i in 0..200 {
            table.insert(make_test_entry(
                i + 1000,
                1,
                &format!("file_{}.txt", i),
                false,
            ));
        }

        // Spawn reader threads
        let mut handles = vec![];
        for _ in 0..5 {
            let table_clone = Arc::clone(&table);
            let handle = thread::spawn(move || {
                for i in 0..200 {
                    let ino = i + 1000;
                    let _ = table_clone.get(ino);
                }
            });
            handles.push(handle);
        }

        // Spawn remover threads
        for start in 0..5 {
            let table_clone = Arc::clone(&table);
            let handle = thread::spawn(move || {
                for i in (start..200).step_by(5) {
                    let ino = i + 1000;
                    let _ = table_clone.remove(ino);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should complete");
        }

        // Verify table is empty (all 200 entries removed)
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_concurrent_children_and_lookup() {
        use std::{sync::Arc, thread};

        let table = Arc::new(InodeTable::new());

        // Create a parent with many children
        table.insert(make_test_entry(1, 1, "root", true));
        for i in 0..100 {
            table.insert(make_test_entry(
                i + 100,
                1,
                &format!("child_{}.txt", i),
                false,
            ));
        }

        // Spawn threads that concurrently call children() and lookup()
        let mut handles = vec![];
        for _ in 0..10 {
            let table_clone = Arc::clone(&table);
            let handle = thread::spawn(move || {
                for i in 0..50 {
                    // Call children()
                    let children = table_clone.children(1);
                    assert!(!children.is_empty());

                    // Call lookup()
                    let name = format!("child_{}.txt", i);
                    let found = table_clone.lookup(1, &name);
                    assert!(found.is_some());
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should complete");
        }

        // Verify table integrity
        assert_eq!(table.len(), 101); // 1 parent + 100 children
    }

    #[test]
    fn test_bidirectional_mapping_consistency() {
        let table = InodeTable::new();
        let item_id = UniqueId::new();
        let entry = make_entry_with_id(42, 1, "test.txt", item_id);

        table.insert(entry);

        // Verify bidirectional consistency
        let ino_from_table = table.get_by_item_id(&item_id).expect("Should exist");
        assert_eq!(ino_from_table, 42);

        let entry_from_table = table.get(42).expect("Should exist");
        assert_eq!(*entry_from_table.item_id(), item_id);

        // Remove and verify both mappings are gone
        table.remove(42);
        assert!(table.get_by_item_id(&item_id).is_none());
        assert!(table.get(42).is_none());
    }

    #[test]
    fn test_default_trait() {
        let table = InodeTable::default();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }
}
