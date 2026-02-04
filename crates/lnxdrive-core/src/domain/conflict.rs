//! Conflict domain entities
//!
//! This module defines types for detecting, tracking, and resolving
//! synchronization conflicts between local and remote file versions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::newtypes::{ConflictId, FileHash, UniqueId};

/// Information about a specific version of a file
///
/// VersionInfo captures the essential metadata needed to compare
/// two versions of the same file and determine if they conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    /// File content hash (quickXorHash for OneDrive)
    hash: FileHash,
    /// File size in bytes
    size_bytes: u64,
    /// When this version was last modified
    modified_at: DateTime<Utc>,
    /// ETag from OneDrive (if available)
    etag: Option<String>,
}

impl VersionInfo {
    /// Creates a new VersionInfo
    ///
    /// # Arguments
    ///
    /// * `hash` - The file content hash
    /// * `size_bytes` - The file size in bytes
    /// * `modified_at` - When the file was last modified
    pub fn new(hash: FileHash, size_bytes: u64, modified_at: DateTime<Utc>) -> Self {
        Self {
            hash,
            size_bytes,
            modified_at,
            etag: None,
        }
    }

    /// Returns the file hash
    pub fn hash(&self) -> &FileHash {
        &self.hash
    }

    /// Returns the file size in bytes
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns when this version was modified
    pub fn modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }

    /// Returns the ETag if available
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Sets the ETag for this version
    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }
}

/// How a conflict should be or was resolved
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Keep the local version, overwriting remote
    KeepLocal,
    /// Keep the remote version, overwriting local
    KeepRemote,
    /// Keep both versions (rename one with conflict suffix)
    KeepBoth,
    /// Requires manual user intervention
    Manual,
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Resolution::KeepLocal => "keep_local",
            Resolution::KeepRemote => "keep_remote",
            Resolution::KeepBoth => "keep_both",
            Resolution::Manual => "manual",
        };
        write!(f, "{}", s)
    }
}

/// Who or what initiated the conflict resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSource {
    /// User manually chose the resolution
    User,
    /// Automatic resolution based on configured policy
    Policy,
    /// System-initiated resolution (e.g., during startup)
    System,
}

impl std::fmt::Display for ResolutionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ResolutionSource::User => "user",
            ResolutionSource::Policy => "policy",
            ResolutionSource::System => "system",
        };
        write!(f, "{}", s)
    }
}

/// A synchronization conflict between local and remote file versions
///
/// Conflicts occur when both the local and remote versions of a file
/// have been modified since the last successful sync. LNXDrive tracks
/// these conflicts and provides mechanisms for resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    /// Unique identifier for this conflict
    id: ConflictId,
    /// The sync item that has conflicting versions
    item_id: UniqueId,
    /// When the conflict was detected
    detected_at: DateTime<Utc>,
    /// Information about the local version
    local_version: VersionInfo,
    /// Information about the remote version
    remote_version: VersionInfo,
    /// How the conflict was resolved (if resolved)
    resolution: Option<Resolution>,
    /// When the conflict was resolved (if resolved)
    resolved_at: Option<DateTime<Utc>>,
    /// Who or what resolved the conflict (if resolved)
    resolved_by: Option<ResolutionSource>,
}

impl Conflict {
    /// Creates a new unresolved conflict
    ///
    /// # Arguments
    ///
    /// * `item_id` - The ID of the sync item with conflicting versions
    /// * `local_version` - Information about the local file version
    /// * `remote_version` - Information about the remote file version
    ///
    /// # Example
    ///
    /// ```
    /// use lnxdrive_core::domain::conflict::{Conflict, VersionInfo};
    /// use lnxdrive_core::domain::newtypes::{UniqueId, FileHash};
    /// use chrono::Utc;
    ///
    /// // quickXorHash is 20 bytes, encoded as 28 Base64 chars
    /// let local = VersionInfo::new(
    ///     FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap(),
    ///     1024,
    ///     Utc::now(),
    /// );
    /// let remote = VersionInfo::new(
    ///     FileHash::new("BBBBBBBBBBBBBBBBBBBBBBBBBBB=".to_string()).unwrap(),
    ///     1048,
    ///     Utc::now(),
    /// );
    ///
    /// let conflict = Conflict::new(UniqueId::new(), local, remote);
    /// assert!(!conflict.is_resolved());
    /// ```
    pub fn new(item_id: UniqueId, local_version: VersionInfo, remote_version: VersionInfo) -> Self {
        Self {
            id: ConflictId::new(),
            item_id,
            detected_at: Utc::now(),
            local_version,
            remote_version,
            resolution: None,
            resolved_at: None,
            resolved_by: None,
        }
    }

    /// Returns the conflict ID
    pub fn id(&self) -> &ConflictId {
        &self.id
    }

    /// Returns the item ID
    pub fn item_id(&self) -> &UniqueId {
        &self.item_id
    }

    /// Returns when the conflict was detected
    pub fn detected_at(&self) -> DateTime<Utc> {
        self.detected_at
    }

    /// Returns information about the local version
    pub fn local_version(&self) -> &VersionInfo {
        &self.local_version
    }

    /// Returns information about the remote version
    pub fn remote_version(&self) -> &VersionInfo {
        &self.remote_version
    }

    /// Returns the resolution if the conflict has been resolved
    pub fn resolution(&self) -> Option<&Resolution> {
        self.resolution.as_ref()
    }

    /// Returns when the conflict was resolved
    pub fn resolved_at(&self) -> Option<DateTime<Utc>> {
        self.resolved_at
    }

    /// Returns who or what resolved the conflict
    pub fn resolved_by(&self) -> Option<&ResolutionSource> {
        self.resolved_by.as_ref()
    }

    /// Returns true if the conflict has been resolved
    pub fn is_resolved(&self) -> bool {
        self.resolution.is_some()
    }

    /// Resolves the conflict with the given resolution and source
    ///
    /// # Arguments
    ///
    /// * `resolution` - How to resolve the conflict
    /// * `source` - Who or what initiated the resolution
    ///
    /// # Returns
    ///
    /// The modified conflict with resolution information set.
    /// If the conflict is already resolved, this is a no-op and
    /// returns the conflict unchanged.
    ///
    /// # Example
    ///
    /// ```
    /// use lnxdrive_core::domain::conflict::{Conflict, VersionInfo, Resolution, ResolutionSource};
    /// use lnxdrive_core::domain::newtypes::{UniqueId, FileHash};
    /// use chrono::Utc;
    ///
    /// let local = VersionInfo::new(
    ///     FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap(),
    ///     1024,
    ///     Utc::now(),
    /// );
    /// let remote = VersionInfo::new(
    ///     FileHash::new("BBBBBBBBBBBBBBBBBBBBBBBBBBB=".to_string()).unwrap(),
    ///     1048,
    ///     Utc::now(),
    /// );
    ///
    /// let conflict = Conflict::new(UniqueId::new(), local, remote)
    ///     .resolve(Resolution::KeepBoth, ResolutionSource::User);
    ///
    /// assert!(conflict.is_resolved());
    /// assert_eq!(conflict.resolution(), Some(&Resolution::KeepBoth));
    /// assert_eq!(conflict.resolved_by(), Some(&ResolutionSource::User));
    /// ```
    pub fn resolve(mut self, resolution: Resolution, source: ResolutionSource) -> Self {
        if self.is_resolved() {
            return self;
        }

        self.resolution = Some(resolution);
        self.resolved_at = Some(Utc::now());
        self.resolved_by = Some(source);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valid quickXorHash Base64 strings (20 bytes = 28 chars with padding)
    const VALID_HASH_1: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const VALID_HASH_2: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBB=";
    const VALID_HASH_3: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCC=";

    fn create_version_info(hash: &str, size: u64) -> VersionInfo {
        VersionInfo::new(
            FileHash::new(hash.to_string()).expect("Invalid test hash"),
            size,
            Utc::now(),
        )
    }

    #[test]
    fn test_version_info_creation() {
        let hash = FileHash::new(VALID_HASH_1.to_string()).unwrap();
        let now = Utc::now();
        let version = VersionInfo::new(hash.clone(), 1024, now);

        assert_eq!(version.hash(), &hash);
        assert_eq!(version.size_bytes(), 1024);
        assert_eq!(version.modified_at(), now);
        assert!(version.etag().is_none());
    }

    #[test]
    fn test_version_info_with_etag() {
        let version = create_version_info(VALID_HASH_1, 2048).with_etag("\"etag123\"");

        assert_eq!(version.etag(), Some("\"etag123\""));
    }

    #[test]
    fn test_resolution_display() {
        assert_eq!(Resolution::KeepLocal.to_string(), "keep_local");
        assert_eq!(Resolution::KeepRemote.to_string(), "keep_remote");
        assert_eq!(Resolution::KeepBoth.to_string(), "keep_both");
        assert_eq!(Resolution::Manual.to_string(), "manual");
    }

    #[test]
    fn test_resolution_serialization() {
        let resolution = Resolution::KeepBoth;
        let json = serde_json::to_string(&resolution).unwrap();
        assert_eq!(json, "\"keep_both\"");

        let deserialized: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, resolution);
    }

    #[test]
    fn test_resolution_source_display() {
        assert_eq!(ResolutionSource::User.to_string(), "user");
        assert_eq!(ResolutionSource::Policy.to_string(), "policy");
        assert_eq!(ResolutionSource::System.to_string(), "system");
    }

    #[test]
    fn test_resolution_source_serialization() {
        let source = ResolutionSource::Policy;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"policy\"");

        let deserialized: ResolutionSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, source);
    }

    #[test]
    fn test_conflict_creation() {
        let item_id = UniqueId::new();
        let local = create_version_info(VALID_HASH_1, 1024);
        let remote = create_version_info(VALID_HASH_2, 1048);

        let conflict = Conflict::new(item_id, local.clone(), remote.clone());

        assert_eq!(conflict.item_id(), &item_id);
        assert_eq!(conflict.local_version().hash(), local.hash());
        assert_eq!(conflict.remote_version().hash(), remote.hash());
        assert!(!conflict.is_resolved());
        assert!(conflict.resolution().is_none());
        assert!(conflict.resolved_at().is_none());
        assert!(conflict.resolved_by().is_none());
    }

    #[test]
    fn test_conflict_resolve() {
        let local = create_version_info(VALID_HASH_1, 1024);
        let remote = create_version_info(VALID_HASH_2, 1048);
        let conflict = Conflict::new(UniqueId::new(), local, remote);

        let resolved = conflict.resolve(Resolution::KeepLocal, ResolutionSource::User);

        assert!(resolved.is_resolved());
        assert_eq!(resolved.resolution(), Some(&Resolution::KeepLocal));
        assert!(resolved.resolved_at().is_some());
        assert_eq!(resolved.resolved_by(), Some(&ResolutionSource::User));
    }

    #[test]
    fn test_conflict_resolve_idempotent() {
        let local = create_version_info(VALID_HASH_1, 1024);
        let remote = create_version_info(VALID_HASH_2, 1048);
        let conflict = Conflict::new(UniqueId::new(), local, remote);

        let resolved = conflict.resolve(Resolution::KeepLocal, ResolutionSource::User);
        let resolved_at = resolved.resolved_at();

        // Try to resolve again with different values
        let resolved_again = resolved.resolve(Resolution::KeepRemote, ResolutionSource::Policy);

        // Should still have the original resolution
        assert_eq!(resolved_again.resolution(), Some(&Resolution::KeepLocal));
        assert_eq!(resolved_again.resolved_by(), Some(&ResolutionSource::User));
        assert_eq!(resolved_again.resolved_at(), resolved_at);
    }

    #[test]
    fn test_conflict_serialization() {
        let local = create_version_info(VALID_HASH_1, 1024);
        let remote = create_version_info(VALID_HASH_2, 1048).with_etag("\"etag456\"");
        let conflict = Conflict::new(UniqueId::new(), local, remote)
            .resolve(Resolution::KeepBoth, ResolutionSource::Policy);

        let json = serde_json::to_string(&conflict).unwrap();
        let deserialized: Conflict = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id(), conflict.id());
        assert_eq!(deserialized.item_id(), conflict.item_id());
        assert_eq!(deserialized.resolution(), conflict.resolution());
        assert_eq!(deserialized.resolved_by(), conflict.resolved_by());
        assert_eq!(
            deserialized.remote_version().etag(),
            conflict.remote_version().etag()
        );
    }

    #[test]
    fn test_version_info_equality() {
        let now = Utc::now();
        let hash1 = FileHash::new(VALID_HASH_1.to_string()).unwrap();
        let hash2 = FileHash::new(VALID_HASH_1.to_string()).unwrap();
        let hash3 = FileHash::new(VALID_HASH_3.to_string()).unwrap();

        let v1 = VersionInfo::new(hash1, 1024, now);
        let v2 = VersionInfo::new(hash2, 1024, now);
        let v3 = VersionInfo::new(hash3, 1024, now);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }
}
