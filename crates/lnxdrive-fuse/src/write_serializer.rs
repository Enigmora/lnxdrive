//! Write operation serialization.
//!
//! Provides `WriteSerializer` to serialize concurrent writes to the same file
//! through SQLite, ensuring data consistency and proper conflict detection.

use chrono::{DateTime, Utc};
use lnxdrive_cache::{pool::DatabasePool, repository::SqliteStateRepository};
use lnxdrive_core::{
    domain::{newtypes::UniqueId, sync_item::ItemState, SyncItem},
    ports::IStateRepository,
};
use tokio::sync::{mpsc, oneshot};

use crate::error::FuseError;

/// Result type for write operations
pub type Result<T> = std::result::Result<T, FuseError>;

// ============================================================================
// WriteOp enum
// ============================================================================

/// Write operations that can be serialized through the WriteSerializer
///
/// Each variant carries the data needed for the operation plus a oneshot
/// sender for returning the result to the caller.
#[derive(Debug)]
pub enum WriteOp {
    /// Update the state of a sync item
    UpdateState {
        item_id: UniqueId,
        state: ItemState,
        reply: oneshot::Sender<Result<()>>,
    },

    /// Update the inode number of a sync item
    UpdateInode {
        item_id: UniqueId,
        inode: u64,
        reply: oneshot::Sender<Result<()>>,
    },

    /// Update the last accessed timestamp of a sync item
    UpdateLastAccessed {
        item_id: UniqueId,
        accessed: DateTime<Utc>,
        reply: oneshot::Sender<Result<()>>,
    },

    /// Update the hydration progress percentage of a sync item
    UpdateHydrationProgress {
        item_id: UniqueId,
        progress: Option<u8>,
        reply: oneshot::Sender<Result<()>>,
    },

    /// Increment the inode counter and return the next available inode
    IncrementInodeCounter { reply: oneshot::Sender<Result<u64>> },

    /// Save a new sync item to the database
    SaveItem {
        item: Box<SyncItem>,
        reply: oneshot::Sender<Result<()>>,
    },

    /// Delete a sync item from the database
    DeleteItem {
        item_id: UniqueId,
        reply: oneshot::Sender<Result<()>>,
    },
}

// ============================================================================
// WriteSerializerHandle
// ============================================================================

/// Handle for sending write operations to the WriteSerializer
///
/// This handle can be cloned and shared across multiple tasks.
/// All operations are processed sequentially by the WriteSerializer task.
#[derive(Clone)]
pub struct WriteSerializerHandle {
    tx: mpsc::Sender<WriteOp>,
}

impl WriteSerializerHandle {
    /// Sends a write operation to update an item's state
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn update_state(&self, item_id: UniqueId, state: ItemState) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::UpdateState {
            item_id,
            state,
            reply: tx,
        };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Sends a write operation to update an item's inode
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn update_inode(&self, item_id: UniqueId, inode: u64) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::UpdateInode {
            item_id,
            inode,
            reply: tx,
        };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Sends a write operation to update an item's last accessed timestamp
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn update_last_accessed(
        &self,
        item_id: UniqueId,
        accessed: DateTime<Utc>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::UpdateLastAccessed {
            item_id,
            accessed,
            reply: tx,
        };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Sends a write operation to update an item's hydration progress
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn update_hydration_progress(
        &self,
        item_id: UniqueId,
        progress: Option<u8>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::UpdateHydrationProgress {
            item_id,
            progress,
            reply: tx,
        };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Allocates a new inode number by incrementing the counter
    ///
    /// Returns the newly allocated inode number.
    pub async fn increment_inode_counter(&self) -> Result<u64> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::IncrementInodeCounter { reply: tx };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Saves a new sync item to the database
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn save_item(&self, item: SyncItem) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::SaveItem {
            item: Box::new(item),
            reply: tx,
        };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }

    /// Deletes a sync item from the database
    ///
    /// Returns when the operation has been processed by the serializer.
    pub async fn delete_item(&self, item_id: UniqueId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let op = WriteOp::DeleteItem { item_id, reply: tx };

        self.tx.send(op).await.map_err(|_| {
            FuseError::DatabaseError("WriteSerializer task has stopped".to_string())
        })?;

        rx.await
            .map_err(|_| FuseError::DatabaseError("WriteSerializer response lost".to_string()))?
    }
}

// ============================================================================
// WriteSerializer
// ============================================================================

/// Serializes write operations to SQLite to prevent SQLITE_BUSY errors
///
/// The WriteSerializer runs as a tokio task that processes write operations
/// sequentially from an mpsc channel. This ensures that all database writes
/// are serialized, preventing concurrent write contention in SQLite.
///
/// # Architecture
///
/// ```text
/// ┌─────────────┐      WriteOp      ┌──────────────────┐
/// │ FUSE thread │ ─────────────────► │ WriteSerializer  │
/// │   (caller)  │                    │      task        │
/// └─────────────┘                    └──────────────────┘
///       │                                      │
///       │          Result via oneshot          │
///       │ ◄────────────────────────────────────┤
///       │                                      │
///       │                                      ▼
///       │                             ┌─────────────────┐
///       │                             │ SqliteStateRepo │
///       │                             └─────────────────┘
/// ```
///
/// # Example
///
/// ```ignore
/// let pool = DatabasePool::new(db_path).await?;
/// let (serializer, handle) = WriteSerializer::new(pool);
///
/// // Spawn the serializer task
/// tokio::spawn(async move {
///     serializer.run().await;
/// });
///
/// // Use the handle to send operations
/// handle.update_state(item_id, ItemState::Hydrated).await?;
/// ```
pub struct WriteSerializer {
    rx: mpsc::Receiver<WriteOp>,
    repository: SqliteStateRepository,
}

impl WriteSerializer {
    /// Creates a new WriteSerializer with the given database pool
    ///
    /// Returns a tuple of:
    /// - The serializer itself (to be spawned as a task)
    /// - A handle for sending write operations
    ///
    /// The caller must spawn the serializer as a tokio task by calling `run()`.
    pub fn new(pool: DatabasePool) -> (Self, WriteSerializerHandle) {
        // Create an mpsc channel for write operations
        // Buffer size of 100 allows reasonable batching without excessive memory use
        let (tx, rx) = mpsc::channel(100);

        let repository = SqliteStateRepository::new(pool.pool().clone());

        let serializer = Self { rx, repository };
        let handle = WriteSerializerHandle { tx };

        (serializer, handle)
    }

    /// Runs the write serializer loop
    ///
    /// This method processes write operations from the channel sequentially,
    /// ensuring no concurrent writes to SQLite. It runs until the channel
    /// is closed (all senders are dropped).
    ///
    /// # Panics
    ///
    /// This method should not panic under normal circumstances. If a database
    /// operation fails, the error is sent back to the caller via the oneshot
    /// channel rather than panicking.
    pub async fn run(mut self) {
        tracing::info!("WriteSerializer task started");

        while let Some(op) = self.rx.recv().await {
            self.process_operation(op).await;
        }

        tracing::info!("WriteSerializer task stopped (all senders dropped)");
    }

    /// Processes a single write operation
    ///
    /// Executes the operation using the repository and sends the result
    /// back to the caller via the oneshot channel.
    async fn process_operation(&self, op: WriteOp) {
        match op {
            WriteOp::UpdateState {
                item_id,
                state,
                reply,
            } => {
                tracing::trace!(?item_id, ?state, "Processing UpdateState");

                // Get the item, update its state, and save it back
                let result = async {
                    let item = self
                        .repository
                        .get_item(&item_id)
                        .await
                        .map_err(|e| FuseError::DatabaseError(e.to_string()))?
                        .ok_or_else(|| {
                            FuseError::NotFound(format!("Item not found: {}", item_id))
                        })?;

                    // Create a new item with updated state
                    let mut updated_item = item;
                    updated_item
                        .transition_to(state)
                        .map_err(|e| FuseError::InvalidArgument(e.to_string()))?;

                    self.repository
                        .save_item(&updated_item)
                        .await
                        .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                    Ok(())
                }
                .await;

                let _ = reply.send(result);
            }

            WriteOp::UpdateInode {
                item_id,
                inode,
                reply,
            } => {
                tracing::trace!(?item_id, inode, "Processing UpdateInode");

                let result = self
                    .repository
                    .update_inode(&item_id, inode)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }

            WriteOp::UpdateLastAccessed {
                item_id,
                accessed,
                reply,
            } => {
                tracing::trace!(?item_id, ?accessed, "Processing UpdateLastAccessed");

                let result = self
                    .repository
                    .update_last_accessed(&item_id, accessed)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }

            WriteOp::UpdateHydrationProgress {
                item_id,
                progress,
                reply,
            } => {
                tracing::trace!(?item_id, ?progress, "Processing UpdateHydrationProgress");

                let result = self
                    .repository
                    .update_hydration_progress(&item_id, progress)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }

            WriteOp::IncrementInodeCounter { reply } => {
                tracing::trace!("Processing IncrementInodeCounter");

                let result = self
                    .repository
                    .get_next_inode()
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }

            WriteOp::SaveItem { item, reply } => {
                tracing::trace!(item_id = ?item.id(), "Processing SaveItem");

                let result = self
                    .repository
                    .save_item(&item)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }

            WriteOp::DeleteItem { item_id, reply } => {
                tracing::trace!(?item_id, "Processing DeleteItem");

                let result = self
                    .repository
                    .delete_item(&item_id)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()));

                let _ = reply.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use lnxdrive_core::domain::{
        newtypes::{Email, RemotePath, SyncPath},
        Account, SyncItem,
    };

    use super::*;

    #[tokio::test]
    async fn test_write_serializer_basic() {
        // Create in-memory database
        let pool = DatabasePool::in_memory().await.unwrap();

        // Create and save a test account first (required for sync items)
        let repo = SqliteStateRepository::new(pool.pool().clone());
        let email = Email::new("test@example.com".to_string()).unwrap();
        let sync_root = SyncPath::new(PathBuf::from("/home/user/OneDrive")).unwrap();
        let account = Account::new(email, "Test User", "drive123", sync_root);
        repo.save_account(&account).await.unwrap();

        // Create a test item
        let item = SyncItem::new(
            SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap(),
            RemotePath::new("/test.txt".to_string()).unwrap(),
            false,
        )
        .unwrap();
        let item_id = *item.id();

        // Save the item
        repo.save_item(&item).await.unwrap();

        // Create the serializer
        let (serializer, handle) = WriteSerializer::new(pool);

        // Spawn the serializer task
        let serializer_task = tokio::spawn(async move {
            serializer.run().await;
        });

        // Test update_state - transition to Hydrating (valid from Online)
        handle
            .update_state(item_id, ItemState::Hydrating)
            .await
            .unwrap();

        // Verify the state was updated
        let updated_item = repo.get_item(&item_id).await.unwrap().unwrap();
        assert_eq!(updated_item.state(), &ItemState::Hydrating);

        // Test another state transition - Hydrating to Hydrated (valid)
        handle
            .update_state(item_id, ItemState::Hydrated)
            .await
            .unwrap();

        let updated_item = repo.get_item(&item_id).await.unwrap().unwrap();
        assert_eq!(updated_item.state(), &ItemState::Hydrated);

        // Test update_inode
        handle.update_inode(item_id, 42).await.unwrap();

        // Test increment_inode_counter
        let inode1 = handle.increment_inode_counter().await.unwrap();
        let inode2 = handle.increment_inode_counter().await.unwrap();
        assert_eq!(inode2, inode1 + 1);

        // Drop the handle to stop the serializer
        drop(handle);

        // Wait for the serializer task to complete
        serializer_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_writes_are_serialized() {
        // Create in-memory database
        let pool = DatabasePool::in_memory().await.unwrap();

        // Create the serializer
        let (serializer, handle) = WriteSerializer::new(pool);

        // Spawn the serializer task
        let serializer_task = tokio::spawn(async move {
            serializer.run().await;
        });

        // Spawn multiple tasks that increment the counter concurrently
        let mut tasks = vec![];
        for _ in 0..10 {
            let handle_clone = handle.clone();
            tasks.push(tokio::spawn(async move {
                handle_clone.increment_inode_counter().await.unwrap()
            }));
        }

        // Collect results
        let mut inodes = vec![];
        for task in tasks {
            inodes.push(task.await.unwrap());
        }

        // All inodes should be unique (serialization ensures no conflicts)
        inodes.sort();
        for i in 0..inodes.len() - 1 {
            assert_ne!(inodes[i], inodes[i + 1], "Inodes should be unique");
        }

        // Drop the handle to stop the serializer
        drop(handle);

        // Wait for the serializer task to complete
        serializer_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_error_propagation() {
        // Create in-memory database
        let pool = DatabasePool::in_memory().await.unwrap();

        // Create and save a test account first (required for sync items)
        let repo = SqliteStateRepository::new(pool.pool().clone());
        let email = Email::new("test@example.com".to_string()).unwrap();
        let sync_root = SyncPath::new(PathBuf::from("/home/user/OneDrive")).unwrap();
        let account = Account::new(email, "Test User", "drive123", sync_root);
        repo.save_account(&account).await.unwrap();

        // Create a test item
        let item = SyncItem::new(
            SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap(),
            RemotePath::new("/test.txt".to_string()).unwrap(),
            false,
        )
        .unwrap();
        let item_id = *item.id();

        // Save the item
        repo.save_item(&item).await.unwrap();

        // Create the serializer
        let (serializer, handle) = WriteSerializer::new(pool);

        // Spawn the serializer task
        let serializer_task = tokio::spawn(async move {
            serializer.run().await;
        });

        // Test 1: Try to update a non-existent item - should propagate NotFound error
        let non_existent_id = UniqueId::new();
        let result = handle
            .update_state(non_existent_id, ItemState::Hydrated)
            .await;

        assert!(result.is_err());
        match result {
            Err(FuseError::NotFound(_)) => {
                // Expected error
            }
            _ => panic!("Expected NotFound error for non-existent item"),
        }

        // Test 2: Try an invalid state transition - should propagate InvalidArgument error
        // The item is currently in Online state, trying to transition to Conflicted is invalid
        // (valid transitions from Online are: Hydrating, Error, Deleted)
        let result = handle.update_state(item_id, ItemState::Conflicted).await;

        assert!(result.is_err());
        match result {
            Err(FuseError::InvalidArgument(_)) => {
                // Expected error
            }
            _ => panic!("Expected InvalidArgument error for invalid state transition"),
        }

        // Test 3: Try to update inode for non-existent item - should propagate error
        let result = handle.update_inode(non_existent_id, 999).await;

        assert!(result.is_err());
        match result {
            Err(FuseError::DatabaseError(_)) => {
                // Expected error (repository returns error for non-existent item)
            }
            _ => panic!("Expected DatabaseError for non-existent item update"),
        }

        // Drop the handle to stop the serializer
        drop(handle);

        // Wait for the serializer task to complete
        serializer_task.await.unwrap();
    }
}
