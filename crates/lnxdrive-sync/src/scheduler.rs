//! Sync scheduler - orchestrates debounced filesystem events into sync triggers
//!
//! The [`SyncScheduler`] sits between the [`FileWatcher`](super::watcher::FileWatcher)
//! and the [`SyncEngine`](super::engine::SyncEngine). It receives raw change events,
//! feeds them through a [`DebouncedChangeQueue`](super::watcher::DebouncedChangeQueue),
//! and signals when a synchronization cycle should begin.
//!
//! ## Flow
//!
//! ```text
//! FileWatcher ──→ mpsc::Receiver ──→ SyncScheduler ──→ sync_requested flag
//!                                        │
//!                                  DebouncedChangeQueue
//! ```
//!
//! The scheduler also supports user-initiated sync requests that bypass
//! the debounce window entirely, useful for "sync now" commands.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::watcher::{ChangeEvent, DebouncedChangeQueue};

// ============================================================================
// T183: SyncScheduler struct
// ============================================================================

/// Schedules sync cycles based on filesystem change events
///
/// Consumes events from a channel, debounces them, and sets a shared
/// atomic flag when settled changes are ready to be processed by the
/// sync engine.
///
/// ## Priority / User-Initiated Sync
///
/// Calling [`request_sync()`](SyncScheduler::request_sync) sets the
/// `sync_requested` flag immediately, bypassing the debounce queue.
/// This allows the CLI or UI to trigger an immediate sync.
pub struct SyncScheduler {
    /// Receiver for change events from the FileWatcher
    change_rx: mpsc::Receiver<ChangeEvent>,
    /// Debounced queue that coalesces rapid-fire events
    queue: DebouncedChangeQueue,
    /// Shared flag indicating that a sync cycle should start
    sync_requested: Arc<AtomicBool>,
    /// How often the scheduler polls the debounced queue for settled events
    poll_interval: Duration,
}

impl SyncScheduler {
    // ========================================================================
    // T183: SyncScheduler::new()
    // ========================================================================

    /// Creates a new `SyncScheduler`
    ///
    /// # Arguments
    /// * `change_rx` - Channel receiver for filesystem change events
    /// * `debounce_delay` - How long a path must be quiet before triggering sync
    /// * `poll_interval` - How often to check the debounce queue for settled events
    ///
    /// # Returns
    /// A tuple of `(SyncScheduler, Arc<AtomicBool>)`. The `AtomicBool` is set
    /// to `true` when changes are ready and the sync engine should run.
    pub fn new(
        change_rx: mpsc::Receiver<ChangeEvent>,
        debounce_delay: Duration,
        poll_interval: Duration,
    ) -> (Self, Arc<AtomicBool>) {
        let sync_requested = Arc::new(AtomicBool::new(false));
        let flag = sync_requested.clone();

        info!(
            debounce_ms = debounce_delay.as_millis() as u64,
            poll_ms = poll_interval.as_millis() as u64,
            "Creating sync scheduler"
        );

        let scheduler = Self {
            change_rx,
            queue: DebouncedChangeQueue::new(debounce_delay),
            sync_requested,
            poll_interval,
        };

        (scheduler, flag)
    }

    // ========================================================================
    // T184: SyncScheduler::enqueue()
    // ========================================================================

    /// Adds a change event to the debounced queue
    ///
    /// The event will be held in the queue until it has been quiet for
    /// the configured debounce delay, at which point it will be emitted
    /// by the next `poll()` cycle.
    ///
    /// # Arguments
    /// * `event` - The filesystem change event to enqueue
    pub fn enqueue(&mut self, event: ChangeEvent) {
        debug!(event = ?event, "Enqueuing change event in scheduler");
        self.queue.push(event);
    }

    /// Requests an immediate sync, bypassing the debounce queue
    ///
    /// This is used for user-initiated "sync now" requests from the CLI
    /// or UI. Sets the `sync_requested` flag directly.
    pub fn request_sync(&self) {
        info!("User-initiated sync requested (bypassing debounce)");
        self.sync_requested.store(true, Ordering::Release);
    }

    // ========================================================================
    // T185: SyncScheduler::run()
    // ========================================================================

    /// Main event loop for the sync scheduler
    ///
    /// Runs indefinitely, performing three concurrent operations via `tokio::select!`:
    ///
    /// 1. **Receive events**: Reads from the change channel and pushes them
    ///    into the debounced queue.
    /// 2. **Poll queue**: Periodically checks the debounce queue for settled
    ///    events and sets the `sync_requested` flag when changes are ready.
    ///
    /// The loop terminates when the change channel is closed (sender dropped).
    pub async fn run(&mut self) {
        info!("Sync scheduler starting");

        let mut poll_timer = tokio::time::interval(self.poll_interval);

        loop {
            tokio::select! {
                // Branch 1: Receive new events from the watcher
                event = self.change_rx.recv() => {
                    match event {
                        Some(change) => {
                            debug!(event = ?change, "Scheduler received change event");
                            self.queue.push(change);
                        }
                        None => {
                            // Channel closed - watcher has been dropped
                            info!("Change channel closed, scheduler shutting down");

                            // Flush any remaining settled events
                            let settled = self.queue.poll();
                            if !settled.is_empty() {
                                info!(
                                    count = settled.len(),
                                    "Flushing remaining settled events before shutdown"
                                );
                                self.sync_requested.store(true, Ordering::Release);
                            }

                            break;
                        }
                    }
                }

                // Branch 2: Periodically poll the debounce queue
                _ = poll_timer.tick() => {
                    let settled = self.queue.poll();
                    if !settled.is_empty() {
                        info!(
                            count = settled.len(),
                            "Settled changes ready for sync"
                        );
                        for event in &settled {
                            debug!(path = %event.path().display(), event = ?event, "Settled");
                        }
                        self.sync_requested.store(true, Ordering::Release);
                    }
                }
            }
        }

        info!("Sync scheduler stopped");
    }

    /// Returns whether a sync has been requested
    ///
    /// This checks the atomic flag without resetting it. Use
    /// [`clear_sync_request`](SyncScheduler::clear_sync_request) to reset.
    pub fn is_sync_requested(&self) -> bool {
        self.sync_requested.load(Ordering::Acquire)
    }

    /// Clears the sync requested flag
    ///
    /// Should be called after the sync engine has started processing.
    pub fn clear_sync_request(&self) {
        self.sync_requested.store(false, Ordering::Release);
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_new_creates_scheduler_with_flag() {
        let (_tx, rx) = mpsc::channel(16);
        let (scheduler, flag) =
            SyncScheduler::new(rx, Duration::from_millis(100), Duration::from_millis(50));

        assert!(!flag.load(Ordering::Acquire));
        assert!(!scheduler.is_sync_requested());
    }

    #[test]
    fn test_request_sync_sets_flag() {
        let (_tx, rx) = mpsc::channel(16);
        let (scheduler, flag) =
            SyncScheduler::new(rx, Duration::from_millis(100), Duration::from_millis(50));

        scheduler.request_sync();
        assert!(flag.load(Ordering::Acquire));
        assert!(scheduler.is_sync_requested());
    }

    #[test]
    fn test_clear_sync_request() {
        let (_tx, rx) = mpsc::channel(16);
        let (scheduler, flag) =
            SyncScheduler::new(rx, Duration::from_millis(100), Duration::from_millis(50));

        scheduler.request_sync();
        assert!(flag.load(Ordering::Acquire));

        scheduler.clear_sync_request();
        assert!(!flag.load(Ordering::Acquire));
    }

    #[test]
    fn test_enqueue_adds_to_queue() {
        let (_tx, rx) = mpsc::channel(16);
        let (mut scheduler, _flag) = SyncScheduler::new(
            rx,
            Duration::from_millis(0), // zero debounce for testing
            Duration::from_millis(50),
        );

        scheduler.enqueue(ChangeEvent::Created(PathBuf::from("/a.txt")));
        assert!(!scheduler.queue.is_empty());
    }

    #[tokio::test]
    async fn test_run_processes_events_and_sets_flag() {
        let (tx, rx) = mpsc::channel(16);
        let (mut scheduler, flag) = SyncScheduler::new(
            rx,
            Duration::from_millis(0), // zero debounce so events settle immediately
            Duration::from_millis(10), // fast polling
        );

        // Send an event
        tx.send(ChangeEvent::Created(PathBuf::from("/test.txt")))
            .await
            .unwrap();

        // Drop the sender so the run loop will eventually exit
        drop(tx);

        // Run the scheduler (will terminate when channel closes)
        scheduler.run().await;

        assert!(flag.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_run_exits_on_channel_close() {
        let (tx, rx) = mpsc::channel(16);
        let (mut scheduler, _flag) =
            SyncScheduler::new(rx, Duration::from_millis(100), Duration::from_millis(10));

        // Drop sender immediately
        drop(tx);

        // Should return without blocking forever
        tokio::time::timeout(Duration::from_secs(2), scheduler.run())
            .await
            .expect("Scheduler should exit when channel closes");
    }

    #[tokio::test]
    async fn test_run_multiple_events_coalesced() {
        let (tx, rx) = mpsc::channel(16);
        let (mut scheduler, flag) = SyncScheduler::new(
            rx,
            Duration::from_millis(0), // zero debounce
            Duration::from_millis(10),
        );

        // Send multiple events for the same path
        tx.send(ChangeEvent::Created(PathBuf::from("/a.txt")))
            .await
            .unwrap();
        tx.send(ChangeEvent::Modified(PathBuf::from("/a.txt")))
            .await
            .unwrap();
        tx.send(ChangeEvent::Modified(PathBuf::from("/a.txt")))
            .await
            .unwrap();

        drop(tx);

        scheduler.run().await;

        // The flag should be set (events were processed)
        assert!(flag.load(Ordering::Acquire));
    }
}
