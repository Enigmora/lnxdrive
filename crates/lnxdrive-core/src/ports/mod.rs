//! Port definitions (hexagonal architecture interfaces)
//!
//! This module defines the port traits that form the boundaries of the
//! hexagonal architecture. Ports are interfaces that the domain core
//! depends on, but whose implementations live in adapter crates.
//!
//! ## Ports Overview
//!
//! - [`ICloudProvider`] - Cloud storage operations (OneDrive, future providers)
//! - [`IStateRepository`] - Persistent storage for sync state, accounts, audit
//! - [`ILocalFileSystem`] - Local filesystem operations and file watching
//! - [`INotificationService`] - Desktop notifications and progress reporting

pub mod cloud_provider;
pub mod local_filesystem;
pub mod notification;
pub mod state_repository;

pub use cloud_provider::{AuthFlow, DeltaItem, DeltaResponse, ICloudProvider, Tokens, UserInfo};
pub use local_filesystem::{FileSystemState, IFileObserver, ILocalFileSystem, WatchHandle};
pub use notification::{INotificationService, Notification, NotificationPriority};
pub use state_repository::{IStateRepository, ItemFilter};
