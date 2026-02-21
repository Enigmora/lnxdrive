//! Extended attributes handler.
//!
//! Handles extended attributes for file metadata, including hydration state
//! indicators (user.lnxdrive.state, user.lnxdrive.size, etc.).
//!
//! ## Supported Extended Attributes
//!
//! - `user.lnxdrive.state` - Current sync/hydration state (Online, Hydrating, Hydrated, etc.)
//! - `user.lnxdrive.size` - File size in bytes
//! - `user.lnxdrive.remote_id` - OneDrive item ID
//! - `user.lnxdrive.progress` - Hydration progress (only during Hydrating state)

use lnxdrive_core::domain::ItemState;

use crate::inode_entry::InodeEntry;

// ============================================================================
// Constants for xattr namespace
// ============================================================================

/// Extended attribute for the current sync/hydration state.
///
/// Values: "Online", "Hydrating", "Hydrated", "Pinned", "Modified", "Conflicted", "Error", "Deleted"
pub const XATTR_STATE: &str = "user.lnxdrive.state";

/// Extended attribute for the file size in bytes.
///
/// Value: decimal string representation of the size (e.g., "1024")
pub const XATTR_SIZE: &str = "user.lnxdrive.size";

/// Extended attribute for the OneDrive remote item ID.
///
/// Value: the OneDrive item identifier string (only present for items synced with OneDrive)
pub const XATTR_REMOTE_ID: &str = "user.lnxdrive.remote_id";

/// Extended attribute for hydration download progress.
///
/// Value: percentage string "0" to "100" (only present during Hydrating state)
pub const XATTR_PROGRESS: &str = "user.lnxdrive.progress";

// ============================================================================
// Helper functions
// ============================================================================

/// Returns a list of all supported extended attribute names.
///
/// This is used to respond to `listxattr` FUSE operations.
///
/// # Returns
///
/// A vector containing all supported xattr names.
#[must_use]
pub fn list_xattrs() -> Vec<&'static str> {
    vec![XATTR_STATE, XATTR_SIZE, XATTR_REMOTE_ID, XATTR_PROGRESS]
}

/// Gets the value of an extended attribute from an inode entry.
///
/// # Arguments
///
/// * `entry` - The inode entry to read the attribute from
/// * `name` - The name of the extended attribute to read
///
/// # Returns
///
/// - `Some(Vec<u8>)` containing the attribute value if the attribute exists and has a value
/// - `None` if the attribute is not recognized or has no value for this entry
///
/// # Supported Attributes
///
/// - `XATTR_STATE` - Always returns the state name as bytes
/// - `XATTR_SIZE` - Always returns the file size as a decimal string in bytes
/// - `XATTR_REMOTE_ID` - Returns the OneDrive ID if present, None otherwise
/// - `XATTR_PROGRESS` - Returns hydration progress (0-100) when state is Hydrating, None otherwise
///
/// # Arguments
///
/// * `entry` - The inode entry to read the attribute from
/// * `name` - The name of the extended attribute to read
/// * `hydration_progress` - Current hydration progress percentage (0-100), if available
#[must_use]
pub fn get_xattr(entry: &InodeEntry, name: &str, hydration_progress: Option<u8>) -> Option<Vec<u8>> {
    match name {
        XATTR_STATE => Some(entry.state().name().as_bytes().to_vec()),
        XATTR_SIZE => Some(entry.size().to_string().as_bytes().to_vec()),
        XATTR_REMOTE_ID => entry.remote_id().map(|r| r.as_str().as_bytes().to_vec()),
        XATTR_PROGRESS => {
            if matches!(entry.state(), ItemState::Hydrating) {
                let pct = hydration_progress.unwrap_or(0);
                Some(pct.to_string().as_bytes().to_vec())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use lnxdrive_core::domain::{RemoteId, UniqueId};

    use super::*;
    use crate::inode_entry::InodeNumber;

    fn create_test_entry(state: ItemState, remote_id: Option<RemoteId>) -> InodeEntry {
        InodeEntry::new(
            InodeNumber::new(2),
            UniqueId::new(),
            remote_id,
            InodeNumber::ROOT,
            "test.txt".to_string(),
            fuser::FileType::RegularFile,
            1024,
            0o644,
            SystemTime::now(),
            SystemTime::now(),
            SystemTime::now(),
            1,
            state,
        )
    }

    #[test]
    fn test_list_xattrs() {
        let xattrs = list_xattrs();
        assert_eq!(xattrs.len(), 4);
        assert!(xattrs.contains(&XATTR_STATE));
        assert!(xattrs.contains(&XATTR_SIZE));
        assert!(xattrs.contains(&XATTR_REMOTE_ID));
        assert!(xattrs.contains(&XATTR_PROGRESS));
    }

    #[test]
    fn test_get_xattr_state() {
        let entry = create_test_entry(ItemState::Online, None);
        let value = get_xattr(&entry, XATTR_STATE, None);
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"Online".to_vec());

        let entry = create_test_entry(ItemState::Hydrated, None);
        let value = get_xattr(&entry, XATTR_STATE, None);
        assert_eq!(value.unwrap(), b"Hydrated".to_vec());

        let entry = create_test_entry(ItemState::Hydrating, None);
        let value = get_xattr(&entry, XATTR_STATE, None);
        assert_eq!(value.unwrap(), b"Hydrating".to_vec());
    }

    #[test]
    fn test_get_xattr_size() {
        let entry = create_test_entry(ItemState::Online, None);
        let value = get_xattr(&entry, XATTR_SIZE, None);
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"1024".to_vec());
    }

    #[test]
    fn test_get_xattr_remote_id_present() {
        let remote_id = RemoteId::new("ABC123XYZ".to_string()).unwrap();
        let entry = create_test_entry(ItemState::Hydrated, Some(remote_id));
        let value = get_xattr(&entry, XATTR_REMOTE_ID, None);
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"ABC123XYZ".to_vec());
    }

    #[test]
    fn test_get_xattr_remote_id_absent() {
        let entry = create_test_entry(ItemState::Online, None);
        let value = get_xattr(&entry, XATTR_REMOTE_ID, None);
        assert!(value.is_none());
    }

    #[test]
    fn test_get_xattr_progress_during_hydrating() {
        let entry = create_test_entry(ItemState::Hydrating, None);
        // Without progress info, defaults to 0
        let value = get_xattr(&entry, XATTR_PROGRESS, None);
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"0".to_vec());

        // With real progress
        let value = get_xattr(&entry, XATTR_PROGRESS, Some(75));
        assert!(value.is_some());
        assert_eq!(value.unwrap(), b"75".to_vec());
    }

    #[test]
    fn test_get_xattr_progress_not_hydrating() {
        let entry = create_test_entry(ItemState::Online, None);
        let value = get_xattr(&entry, XATTR_PROGRESS, None);
        assert!(value.is_none());

        let entry = create_test_entry(ItemState::Hydrated, None);
        let value = get_xattr(&entry, XATTR_PROGRESS, Some(100));
        assert!(value.is_none());

        let entry = create_test_entry(ItemState::Pinned, None);
        let value = get_xattr(&entry, XATTR_PROGRESS, None);
        assert!(value.is_none());
    }

    #[test]
    fn test_get_xattr_unknown() {
        let entry = create_test_entry(ItemState::Online, None);
        let value = get_xattr(&entry, "user.unknown", None);
        assert!(value.is_none());

        let value = get_xattr(&entry, "security.selinux", None);
        assert!(value.is_none());
    }

    #[test]
    fn test_constants() {
        assert_eq!(XATTR_STATE, "user.lnxdrive.state");
        assert_eq!(XATTR_SIZE, "user.lnxdrive.size");
        assert_eq!(XATTR_REMOTE_ID, "user.lnxdrive.remote_id");
        assert_eq!(XATTR_PROGRESS, "user.lnxdrive.progress");
    }
}
