//! Notification service port (driven/secondary port)
//!
//! This module defines the interface for sending desktop notifications
//! and displaying sync progress to the user. Implementations may use
//! D-Bus (libnotify), desktop-environment-specific APIs, or a fallback
//! mechanism.
//!
//! ## Design Notes
//!
//! - Uses `anyhow::Result` because notification delivery is adapter-specific.
//! - Notifications are fire-and-forget; the caller does not wait for
//!   user interaction.
//! - Progress reporting uses a `progress_id` to allow updating and
//!   clearing specific progress indicators.

use serde::{Deserialize, Serialize};

// ============================================================================
// T059: Notification struct and NotificationPriority enum
// ============================================================================

/// Priority level for a notification
///
/// Maps to urgency levels in notification systems (e.g., libnotify urgency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    /// Low priority, may not be shown immediately
    Low,
    /// Normal priority, shown in the notification area
    Normal,
    /// High priority, may trigger a banner or sound
    High,
    /// Critical priority, persists until acknowledged
    Critical,
}

impl Default for NotificationPriority {
    fn default() -> Self {
        NotificationPriority::Normal
    }
}

impl std::fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NotificationPriority::Low => "low",
            NotificationPriority::Normal => "normal",
            NotificationPriority::High => "high",
            NotificationPriority::Critical => "critical",
        };
        write!(f, "{}", s)
    }
}

/// A notification to display to the user
///
/// Contains the content and metadata for a desktop notification.
/// Implementations may map `category` to notification categories
/// supported by the desktop environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Title of the notification (short, descriptive)
    pub title: String,
    /// Body text with details about the event
    pub body: String,
    /// Priority level affecting how the notification is displayed
    pub priority: NotificationPriority,
    /// Category for grouping/filtering (e.g., "sync", "conflict", "error")
    pub category: String,
}

impl Notification {
    /// Creates a new notification with the given title and body
    ///
    /// Uses `Normal` priority and an empty category by default.
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            priority: NotificationPriority::Normal,
            category: String::new(),
        }
    }

    /// Sets the priority level
    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the category
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Creates a sync-related notification
    pub fn sync(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body).with_category("sync")
    }

    /// Creates an error notification with High priority
    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body)
            .with_priority(NotificationPriority::High)
            .with_category("error")
    }

    /// Creates a conflict notification with High priority
    pub fn conflict(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body)
            .with_priority(NotificationPriority::High)
            .with_category("conflict")
    }
}

// ============================================================================
// T060: INotificationService trait
// ============================================================================

/// Port trait for desktop notification and progress reporting
///
/// This is the interface for communicating sync status and events to the
/// user through the desktop environment's notification system.
///
/// ## Implementation Notes
///
/// - `notify` sends a one-shot notification (toast/banner).
/// - `show_progress` creates or updates a progress indicator identified
///   by `progress_id`. The `percent` value ranges from 0.0 to 100.0.
/// - `clear_progress` removes a progress indicator when the operation
///   is complete or cancelled.
/// - Implementations should gracefully handle notification failures
///   (e.g., missing notification daemon) without crashing.
#[async_trait::async_trait]
pub trait INotificationService: Send + Sync {
    /// Sends a desktop notification to the user
    ///
    /// # Arguments
    /// * `notification` - The notification content and metadata
    async fn notify(&self, notification: &Notification) -> anyhow::Result<()>;

    /// Shows or updates a progress indicator
    ///
    /// If a progress indicator with the given `progress_id` already exists,
    /// it is updated. Otherwise, a new one is created.
    ///
    /// # Arguments
    /// * `progress_id` - Unique identifier for this progress indicator
    /// * `title` - Description of the operation in progress
    /// * `percent` - Completion percentage (0.0 to 100.0)
    async fn show_progress(
        &self,
        progress_id: &str,
        title: &str,
        percent: f64,
    ) -> anyhow::Result<()>;

    /// Clears (removes) a progress indicator
    ///
    /// # Arguments
    /// * `progress_id` - The identifier of the progress indicator to remove
    async fn clear_progress(&self, progress_id: &str) -> anyhow::Result<()>;
}
