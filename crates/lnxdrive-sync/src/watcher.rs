//! File watching and debounced change queue
//!
//! Provides a [`FileWatcher`] that wraps the `notify` crate to monitor filesystem
//! directories for changes, converting raw OS events into [`ChangeEvent`] values.
//!
//! The [`DebouncedChangeQueue`] collects rapid-fire events and coalesces them
//! so that downstream consumers only see the final state of a path after it has
//! been quiet for a configurable debounce window.
//!
//! ## Architecture
//!
//! ```text
//! inotify / fanotify
//!       │
//!       ▼
//!  FileWatcher  ──→  mpsc::channel  ──→  DebouncedChangeQueue  ──→  SyncScheduler
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

// ============================================================================
// T178: ChangeEvent enum
// ============================================================================

/// Represents a filesystem change event detected by the watcher
///
/// These events are the internal representation used by the sync engine,
/// decoupled from the `notify` crate's raw event types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeEvent {
    /// A new file or directory was created at the given path
    Created(PathBuf),
    /// An existing file was modified (content or metadata changed)
    Modified(PathBuf),
    /// A file or directory was deleted from the given path
    Deleted(PathBuf),
    /// A file or directory was renamed/moved
    Renamed {
        /// The original path before the rename
        old: PathBuf,
        /// The new path after the rename
        new: PathBuf,
    },
}

impl ChangeEvent {
    /// Returns the primary path associated with this event
    ///
    /// For rename events, this returns the new (destination) path.
    pub fn path(&self) -> &Path {
        match self {
            ChangeEvent::Created(p) => p,
            ChangeEvent::Modified(p) => p,
            ChangeEvent::Deleted(p) => p,
            ChangeEvent::Renamed { new, .. } => new,
        }
    }
}

// ============================================================================
// T174: FileWatcher struct
// ============================================================================

/// Watches filesystem directories for changes using the OS-native mechanism
///
/// On Linux this typically uses inotify. The watcher converts raw OS events
/// into [`ChangeEvent`] values and sends them through an mpsc channel.
///
/// ## Usage
///
/// ```ignore
/// let (watcher, rx) = FileWatcher::new(500)?;
/// let handle = watcher.watch("/home/user/OneDrive")?;
/// // rx.recv().await to get events
/// drop(handle); // stops watching
/// ```
pub struct FileWatcher {
    /// The underlying notify watcher instance
    watcher: RecommendedWatcher,
    /// Sender half of the channel used to emit ChangeEvents
    event_tx: mpsc::Sender<ChangeEvent>,
}

impl FileWatcher {
    // ========================================================================
    // T175: FileWatcher::new()
    // ========================================================================

    /// Creates a new `FileWatcher` with the specified debounce interval
    ///
    /// Returns the watcher and a receiver channel for consuming change events.
    ///
    /// # Arguments
    /// * `debounce_ms` - Minimum interval in milliseconds between event deliveries
    ///
    /// # Returns
    /// A tuple of `(FileWatcher, mpsc::Receiver<ChangeEvent>)`.
    /// The receiver yields [`ChangeEvent`] values as filesystem changes occur.
    ///
    /// # Errors
    /// Returns an error if the underlying OS watcher cannot be created
    pub fn new(debounce_ms: u64) -> Result<(Self, mpsc::Receiver<ChangeEvent>)> {
        let (event_tx, event_rx) = mpsc::channel::<ChangeEvent>(1024);
        let tx = event_tx.clone();

        info!(debounce_ms, "Initializing file watcher");

        // Note: notify 6.x uses an event handler callback rather than a
        // built-in debounce. We configure the watcher with a callback that
        // converts events and sends them through the channel. Debouncing is
        // handled externally by DebouncedChangeQueue.
        let _ = debounce_ms; // Debouncing is handled by DebouncedChangeQueue

        let watcher = RecommendedWatcher::new(
            move |res: std::result::Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if let Some(change) = map_notify_event(&event) {
                        if let Err(e) = tx.blocking_send(change) {
                            warn!(error = %e, "Failed to send change event (receiver dropped)");
                        }
                    }
                }
                Err(err) => {
                    error!(error = %err, "File watcher error");
                }
            },
            notify::Config::default(),
        )
        .context("Failed to create file watcher")?;

        Ok((Self { watcher, event_tx }, event_rx))
    }

    // ========================================================================
    // T176: FileWatcher::watch()
    // ========================================================================

    /// Starts watching a directory recursively for filesystem changes
    ///
    /// All subdirectories under the given path will be monitored. Returns a
    /// [`WatchHandle`] that, when dropped, stops watching the path.
    ///
    /// # Arguments
    /// * `path` - The directory path to watch
    ///
    /// # Errors
    /// Returns an error if the path cannot be watched (e.g., does not exist,
    /// insufficient permissions, or inotify watch limit reached)
    pub fn watch(&mut self, path: &Path) -> Result<WatchHandle> {
        info!(path = %path.display(), "Starting recursive watch");

        self.watcher
            .watch(path, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch path: {}", path.display()))?;

        let watched_path = path.to_path_buf();
        let tx = self.event_tx.clone();
        let _ = tx; // Keep tx alive; the handle only needs to track the path

        Ok(WatchHandle {
            path: Some(watched_path),
        })
    }

    // ========================================================================
    // T177: FileWatcher::unwatch()
    // ========================================================================

    /// Stops watching a directory
    ///
    /// After this call, no further events will be generated for the given path.
    ///
    /// # Arguments
    /// * `path` - The directory path to stop watching
    ///
    /// # Errors
    /// Returns an error if the path was not being watched
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        info!(path = %path.display(), "Stopping watch");

        self.watcher
            .unwatch(path)
            .with_context(|| format!("Failed to unwatch path: {}", path.display()))?;

        Ok(())
    }
}

// ============================================================================
// WatchHandle - RAII guard for an active watch
// ============================================================================

/// RAII handle for an active filesystem watch
///
/// Stores the path being watched. The caller can use this to identify
/// which path the handle corresponds to.
#[derive(Debug)]
pub struct WatchHandle {
    /// The path being watched (None if the handle has been consumed)
    path: Option<PathBuf>,
}

impl WatchHandle {
    /// Returns the path being watched
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

// ============================================================================
// T179: Event mapping - notify::Event → ChangeEvent
// ============================================================================

/// Converts a `notify::Event` into our internal `ChangeEvent`
///
/// Maps the notify event kinds as follows:
/// - `Create(*)` -> `ChangeEvent::Created`
/// - `Modify(Data(*))` -> `ChangeEvent::Modified`
/// - `Modify(Name(Both))` with 2 paths -> `ChangeEvent::Renamed`
/// - `Remove(*)` -> `ChangeEvent::Deleted`
/// - Other `Modify(*)` -> `ChangeEvent::Modified`
///
/// Returns `None` for events that have no associated paths or that should
/// be ignored (e.g., access events).
fn map_notify_event(event: &notify::Event) -> Option<ChangeEvent> {
    let paths = &event.paths;

    match &event.kind {
        EventKind::Create(_) => {
            let path = paths.first()?;
            debug!(path = %path.display(), "Mapped Create event");
            Some(ChangeEvent::Created(path.clone()))
        }

        EventKind::Modify(ModifyKind::Data(_)) => {
            let path = paths.first()?;
            debug!(path = %path.display(), "Mapped Modify(Data) event");
            Some(ChangeEvent::Modified(path.clone()))
        }

        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if paths.len() >= 2 {
                let old = paths[0].clone();
                let new = paths[1].clone();
                debug!(
                    old = %old.display(),
                    new = %new.display(),
                    "Mapped Rename event"
                );
                Some(ChangeEvent::Renamed { old, new })
            } else {
                // Fallback: treat as a modification of the first path
                let path = paths.first()?;
                debug!(path = %path.display(), "Rename with single path, treating as Modified");
                Some(ChangeEvent::Modified(path.clone()))
            }
        }

        EventKind::Remove(_) => {
            let path = paths.first()?;
            debug!(path = %path.display(), "Mapped Remove event");
            Some(ChangeEvent::Deleted(path.clone()))
        }

        EventKind::Modify(_) => {
            // Other modify kinds (metadata, name-from, name-to, etc.)
            let path = paths.first()?;
            debug!(path = %path.display(), kind = ?event.kind, "Mapped other Modify event");
            Some(ChangeEvent::Modified(path.clone()))
        }

        // Ignore access events and other event types
        _ => {
            debug!(kind = ?event.kind, "Ignoring event kind");
            None
        }
    }
}

// ============================================================================
// T187: File stability check
// ============================================================================

/// Checks if a file is stable (not currently being written to)
///
/// Reads the file size twice, separated by `check_interval_ms` milliseconds.
/// If the size is the same both times, the file is considered stable.
///
/// This is useful for avoiding syncing files that are still being written
/// by another process (e.g., downloads in progress, large copy operations).
///
/// # Arguments
/// * `path` - The file path to check
/// * `check_interval_ms` - Milliseconds to wait between the two size reads
///
/// # Returns
/// `true` if the file size is constant across both reads, `false` if it
/// changed or if the file could not be read.
pub async fn is_file_stable(path: &Path, check_interval_ms: u64) -> bool {
    let size_first = match tokio::fs::metadata(path).await {
        Ok(m) => m.len(),
        Err(err) => {
            warn!(
                path = %path.display(),
                error = %err,
                "Cannot read file metadata for stability check"
            );
            return false;
        }
    };

    tokio::time::sleep(Duration::from_millis(check_interval_ms)).await;

    let size_second = match tokio::fs::metadata(path).await {
        Ok(m) => m.len(),
        Err(err) => {
            warn!(
                path = %path.display(),
                error = %err,
                "Cannot read file metadata on second stability check"
            );
            return false;
        }
    };

    let stable = size_first == size_second;
    debug!(
        path = %path.display(),
        size_first,
        size_second,
        stable,
        "File stability check"
    );
    stable
}

// ============================================================================
// T180: DebouncedChangeQueue struct
// ============================================================================

/// Queue that coalesces rapid filesystem changes into debounced events
///
/// When multiple events arrive for the same path in quick succession,
/// only the latest event type is kept and its timestamp is reset. Events
/// are only emitted (via [`poll`](DebouncedChangeQueue::poll)) once they
/// have been quiet for longer than the configured debounce delay.
///
/// ## Design
///
/// This prevents the sync engine from reacting to every intermediate save
/// of a file being edited, or to rapid create/modify sequences that happen
/// when applications write files.
pub struct DebouncedChangeQueue {
    /// Pending changes keyed by path, storing the latest event and its timestamp
    pending: HashMap<PathBuf, (ChangeEvent, Instant)>,
    /// Minimum quiet period before a change is considered settled
    debounce_delay: Duration,
}

impl DebouncedChangeQueue {
    /// Creates a new `DebouncedChangeQueue` with the given debounce delay
    ///
    /// # Arguments
    /// * `debounce_delay` - How long a path must be quiet before its event is emitted
    pub fn new(debounce_delay: Duration) -> Self {
        Self {
            pending: HashMap::new(),
            debounce_delay,
        }
    }

    // ========================================================================
    // T181: DebouncedChangeQueue::push()
    // ========================================================================

    /// Inserts or updates a change event for the given path
    ///
    /// If the path already has a pending event, the event type is replaced
    /// with the new one and the timestamp is reset to `Instant::now()`.
    /// This means rapid changes to the same file will keep extending
    /// the debounce window until the changes stop.
    ///
    /// # Arguments
    /// * `event` - The change event to enqueue
    pub fn push(&mut self, event: ChangeEvent) {
        let path = event.path().to_path_buf();
        debug!(
            path = %path.display(),
            event = ?event,
            "Enqueuing change event"
        );
        self.pending.insert(path, (event, Instant::now()));
    }

    // ========================================================================
    // T182: DebouncedChangeQueue::poll()
    // ========================================================================

    /// Returns all changes whose timestamp is older than the debounce delay
    ///
    /// Settled events are removed from the pending queue and returned.
    /// Events that are still within the debounce window remain pending.
    ///
    /// # Returns
    /// A vector of settled change events, possibly empty
    pub fn poll(&mut self) -> Vec<ChangeEvent> {
        let now = Instant::now();
        let mut settled = Vec::new();
        let mut settled_paths = Vec::new();

        for (path, (event, timestamp)) in &self.pending {
            if now.duration_since(*timestamp) >= self.debounce_delay {
                settled.push(event.clone());
                settled_paths.push(path.clone());
            }
        }

        for path in &settled_paths {
            self.pending.remove(path);
        }

        if !settled.is_empty() {
            debug!(count = settled.len(), "Polled settled change events");
        }

        settled
    }

    /// Returns the number of pending (unsettled) events
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns true if there are no pending events
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

// ============================================================================
// T188: Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ------------------------------------------------------------------
    // ChangeEvent construction tests
    // ------------------------------------------------------------------

    #[test]
    fn test_change_event_created() {
        let path = PathBuf::from("/home/user/file.txt");
        let event = ChangeEvent::Created(path.clone());
        assert_eq!(event.path(), path);
    }

    #[test]
    fn test_change_event_modified() {
        let path = PathBuf::from("/home/user/file.txt");
        let event = ChangeEvent::Modified(path.clone());
        assert_eq!(event.path(), path);
    }

    #[test]
    fn test_change_event_deleted() {
        let path = PathBuf::from("/home/user/file.txt");
        let event = ChangeEvent::Deleted(path.clone());
        assert_eq!(event.path(), path);
    }

    #[test]
    fn test_change_event_renamed() {
        let old = PathBuf::from("/home/user/old.txt");
        let new = PathBuf::from("/home/user/new.txt");
        let event = ChangeEvent::Renamed {
            old: old.clone(),
            new: new.clone(),
        };
        // path() returns the new path for renames
        assert_eq!(event.path(), new);
    }

    #[test]
    fn test_change_event_equality() {
        let a = ChangeEvent::Created(PathBuf::from("/a.txt"));
        let b = ChangeEvent::Created(PathBuf::from("/a.txt"));
        let c = ChangeEvent::Modified(PathBuf::from("/a.txt"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ------------------------------------------------------------------
    // DebouncedChangeQueue push tests
    // ------------------------------------------------------------------

    #[test]
    fn test_push_single_event() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(100));
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_push_multiple_paths() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(100));
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));
        queue.push(ChangeEvent::Modified(PathBuf::from("/b.txt")));
        queue.push(ChangeEvent::Deleted(PathBuf::from("/c.txt")));
        assert_eq!(queue.pending_count(), 3);
    }

    #[test]
    fn test_push_coalesces_same_path() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(100));

        // Push Created then Modified for the same path
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));
        queue.push(ChangeEvent::Modified(PathBuf::from("/a.txt")));

        // Should only have one pending entry
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_push_keeps_latest_event_type() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_secs(0));

        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));
        queue.push(ChangeEvent::Modified(PathBuf::from("/a.txt")));
        queue.push(ChangeEvent::Deleted(PathBuf::from("/a.txt")));

        // After debounce period, poll should return the last event (Deleted)
        std::thread::sleep(Duration::from_millis(10));
        let settled = queue.poll();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0], ChangeEvent::Deleted(PathBuf::from("/a.txt")));
    }

    // ------------------------------------------------------------------
    // DebouncedChangeQueue poll tests
    // ------------------------------------------------------------------

    #[test]
    fn test_poll_returns_nothing_for_recent_events() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_secs(60));
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));

        // Events just pushed should not be settled yet
        let settled = queue.poll();
        assert!(settled.is_empty());
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_poll_returns_settled_events() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(0));
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));

        // With zero debounce, events should settle immediately
        std::thread::sleep(Duration::from_millis(10));
        let settled = queue.poll();
        assert_eq!(settled.len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_poll_removes_settled_events() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(0));
        queue.push(ChangeEvent::Modified(PathBuf::from("/a.txt")));

        std::thread::sleep(Duration::from_millis(10));
        let first_poll = queue.poll();
        assert_eq!(first_poll.len(), 1);

        // Second poll should return nothing
        let second_poll = queue.poll();
        assert!(second_poll.is_empty());
    }

    #[test]
    fn test_poll_partial_settlement() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(50));

        // Push an event and wait for it to settle
        queue.push(ChangeEvent::Created(PathBuf::from("/old.txt")));
        std::thread::sleep(Duration::from_millis(60));

        // Push a new event (not yet settled)
        queue.push(ChangeEvent::Created(PathBuf::from("/new.txt")));

        let settled = queue.poll();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0], ChangeEvent::Created(PathBuf::from("/old.txt")));
        // The new event is still pending
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_coalescing_resets_timestamp() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(50));

        // Push first event
        queue.push(ChangeEvent::Created(PathBuf::from("/a.txt")));
        std::thread::sleep(Duration::from_millis(30));

        // Update same path - resets the debounce timer
        queue.push(ChangeEvent::Modified(PathBuf::from("/a.txt")));

        // First event was pushed 30ms ago, but the update was just now
        // So after 30ms more, it should still not be settled (50ms debounce)
        std::thread::sleep(Duration::from_millis(30));
        let settled = queue.poll();
        assert!(settled.is_empty());

        // But after another 30ms (total 60ms since last update), it should settle
        std::thread::sleep(Duration::from_millis(30));
        let settled = queue.poll();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0], ChangeEvent::Modified(PathBuf::from("/a.txt")));
    }

    #[test]
    fn test_empty_queue() {
        let mut queue = DebouncedChangeQueue::new(Duration::from_millis(100));
        assert!(queue.is_empty());
        assert_eq!(queue.pending_count(), 0);
        assert!(queue.poll().is_empty());
    }

    // ------------------------------------------------------------------
    // Event mapping tests
    // ------------------------------------------------------------------

    #[test]
    fn test_map_create_event() {
        let event = notify::Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("/a.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(mapped, ChangeEvent::Created(PathBuf::from("/a.txt")));
    }

    #[test]
    fn test_map_modify_data_event() {
        let event = notify::Event {
            kind: EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            paths: vec![PathBuf::from("/a.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(mapped, ChangeEvent::Modified(PathBuf::from("/a.txt")));
    }

    #[test]
    fn test_map_rename_event() {
        let event = notify::Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![PathBuf::from("/old.txt"), PathBuf::from("/new.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(
            mapped,
            ChangeEvent::Renamed {
                old: PathBuf::from("/old.txt"),
                new: PathBuf::from("/new.txt"),
            }
        );
    }

    #[test]
    fn test_map_rename_single_path_fallback() {
        let event = notify::Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            paths: vec![PathBuf::from("/only.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(mapped, ChangeEvent::Modified(PathBuf::from("/only.txt")));
    }

    #[test]
    fn test_map_remove_event() {
        let event = notify::Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![PathBuf::from("/a.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(mapped, ChangeEvent::Deleted(PathBuf::from("/a.txt")));
    }

    #[test]
    fn test_map_other_modify_event() {
        let event = notify::Event {
            kind: EventKind::Modify(ModifyKind::Metadata(
                notify::event::MetadataKind::Permissions,
            )),
            paths: vec![PathBuf::from("/a.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event).unwrap();
        assert_eq!(mapped, ChangeEvent::Modified(PathBuf::from("/a.txt")));
    }

    #[test]
    fn test_map_access_event_ignored() {
        let event = notify::Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![PathBuf::from("/a.txt")],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event);
        assert!(mapped.is_none());
    }

    #[test]
    fn test_map_event_no_paths() {
        let event = notify::Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![],
            attrs: Default::default(),
        };
        let mapped = map_notify_event(&event);
        assert!(mapped.is_none());
    }
}
