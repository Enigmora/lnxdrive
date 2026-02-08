//! Local filesystem adapter (secondary/driven adapter)
//!
//! Implements [`ILocalFileSystem`] using `tokio::fs` for async file operations.
//!
//! ## Design Decisions
//!
//! - **Atomic writes**: Uses write-to-temp + rename to avoid partial writes
//!   on crash or power loss.
//! - **Lock detection**: Attempts an exclusive open via `spawn_blocking` to
//!   check whether another process holds the file.
//! - **quickXorHash**: Implements the OneDrive-compatible hash so local and
//!   remote hashes can be compared without downloading content.
//! - **Watch stub**: Returns a no-op `WatchHandle`; real inotify-based
//!   watching is planned for Phase 6.

use std::io::ErrorKind;

use base64::Engine;
use chrono::DateTime;
use lnxdrive_core::{
    domain::newtypes::{FileHash, SyncPath},
    ports::local_filesystem::{FileSystemState, ILocalFileSystem, WatchHandle},
};
use tracing::{debug, instrument};

// ============================================================================
// T144: LocalFileSystemAdapter struct
// ============================================================================

/// Adapter that bridges the [`ILocalFileSystem`] port to the real filesystem.
///
/// This is a zero-sized struct because all operations derive their context
/// from the [`SyncPath`] arguments. Configuration (e.g. sync root) lives
/// at a higher layer.
#[derive(Debug, Clone, Default)]
pub struct LocalFileSystemAdapter;

impl LocalFileSystemAdapter {
    /// Create a new `LocalFileSystemAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

// ============================================================================
// QuickXorHash implementation
// ============================================================================

/// OneDrive-compatible quickXorHash algorithm.
///
/// The algorithm works on a 160-bit (20-byte) hash state. For each input
/// byte, it is XOR-ed into the state at the current *bit* position and the
/// position advances by 11 bits (mod 160). After processing all input bytes
/// the total file length (as a little-endian `u64`) is XOR-ed into the
/// first 8 bytes of the state. The final 20-byte result is base64-encoded.
struct QuickXorHash {
    data: [u8; 20],
    shift: usize,
    length: u64,
}

impl QuickXorHash {
    /// Width of the hash in bits.
    const WIDTH_BITS: usize = 160;

    /// Number of bits the position advances per input byte.
    const SHIFT_STEP: usize = 11;

    fn new() -> Self {
        Self {
            data: [0u8; 20],
            shift: 0,
            length: 0,
        }
    }

    fn update(&mut self, input: &[u8]) {
        for &byte in input {
            let byte_pos = self.shift / 8;
            let bit_offset = self.shift % 8;

            self.data[byte_pos % 20] ^= byte << bit_offset;
            if bit_offset > 0 {
                self.data[(byte_pos + 1) % 20] ^= byte >> (8 - bit_offset);
            }

            self.shift = (self.shift + Self::SHIFT_STEP) % Self::WIDTH_BITS;
        }
        self.length += input.len() as u64;
    }

    fn finalize(mut self) -> [u8; 20] {
        // XOR the total length (little-endian u64) into the first 8 bytes.
        let length_bytes = self.length.to_le_bytes();
        for (i, &lb) in length_bytes.iter().enumerate() {
            self.data[i] ^= lb;
        }
        self.data
    }
}

// ============================================================================
// T145-T149: ILocalFileSystem implementation
// ============================================================================

#[async_trait::async_trait]
impl ILocalFileSystem for LocalFileSystemAdapter {
    // T145: read_file - async file read
    #[instrument(skip(self), fields(path = %path))]
    async fn read_file(&self, path: &SyncPath) -> anyhow::Result<Vec<u8>> {
        debug!("reading file");
        let data = tokio::fs::read(path.as_path()).await?;
        debug!(bytes = data.len(), "file read complete");
        Ok(data)
    }

    // T146: write_file - atomic write via temp + rename
    #[instrument(skip(self, data), fields(path = %path, bytes = data.len()))]
    async fn write_file(&self, path: &SyncPath, data: &[u8]) -> anyhow::Result<()> {
        let target = path.as_path();

        // Ensure parent directory exists.
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write to a temporary file in the same directory so rename is atomic
        // (same filesystem).
        let tmp_path = {
            let mut p = target.as_os_str().to_owned();
            p.push(".tmp");
            std::path::PathBuf::from(p)
        };

        debug!(?tmp_path, "writing to temporary file");
        tokio::fs::write(&tmp_path, data).await?;

        // Atomic rename.
        debug!("renaming temporary file to target");
        tokio::fs::rename(&tmp_path, target).await?;

        debug!("write complete");
        Ok(())
    }

    // T147: delete_file - remove file or directory recursively
    #[instrument(skip(self), fields(path = %path))]
    async fn delete_file(&self, path: &SyncPath) -> anyhow::Result<()> {
        let p = path.as_path();
        let metadata = tokio::fs::metadata(p).await?;

        if metadata.is_dir() {
            debug!("removing directory recursively");
            tokio::fs::remove_dir_all(p).await?;
        } else {
            debug!("removing file");
            tokio::fs::remove_file(p).await?;
        }

        debug!("delete complete");
        Ok(())
    }

    // T148: get_state - stat file, detect locks
    #[instrument(skip(self), fields(path = %path))]
    async fn get_state(&self, path: &SyncPath) -> anyhow::Result<FileSystemState> {
        let p = path.as_path().clone();

        let metadata = match tokio::fs::metadata(&p).await {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                debug!("path not found");
                return Ok(FileSystemState::not_found());
            }
            Err(e) => return Err(e.into()),
        };

        let is_file = metadata.is_file();
        let size = metadata.len();

        // Convert system modified time to DateTime<Utc>.
        let modified = metadata.modified().ok().and_then(|st| {
            st.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|dur| DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos()))
        });

        // Detect lock by attempting an exclusive write-open from a blocking
        // thread.  If the open fails with WouldBlock or PermissionDenied we
        // treat the file as locked.
        let is_locked = if is_file {
            let p_owned = p.to_path_buf();
            tokio::task::spawn_blocking(move || {
                use std::fs::OpenOptions;
                match OpenOptions::new().write(true).open(&p_owned) {
                    Ok(_) => false,
                    Err(e)
                        if e.kind() == ErrorKind::WouldBlock
                            || e.kind() == ErrorKind::PermissionDenied =>
                    {
                        true
                    }
                    // Any other error (e.g. file disappeared) - not locked.
                    Err(_) => false,
                }
            })
            .await?
        } else {
            false
        };

        debug!(exists = true, is_file, size, is_locked, "state retrieved");

        Ok(FileSystemState {
            exists: true,
            is_file,
            size,
            modified,
            is_locked,
        })
    }

    // T149: compute_hash - quickXorHash matching OneDrive format
    #[instrument(skip(self), fields(path = %path))]
    async fn compute_hash(&self, path: &SyncPath) -> anyhow::Result<FileHash> {
        debug!("computing quickXorHash");
        let data = tokio::fs::read(path.as_path()).await?;

        let mut hasher = QuickXorHash::new();
        hasher.update(&data);
        let hash_bytes = hasher.finalize();

        let encoded = base64::engine::general_purpose::STANDARD.encode(hash_bytes);
        debug!(hash = %encoded, "hash computed");

        Ok(FileHash::new(encoded)?)
    }

    // create_directory
    #[instrument(skip(self), fields(path = %path))]
    async fn create_directory(&self, path: &SyncPath) -> anyhow::Result<()> {
        debug!("creating directory");
        tokio::fs::create_dir_all(path.as_path()).await?;
        debug!("directory created");
        Ok(())
    }

    // watch - no-op stub; real filesystem watching is handled by FileWatcher (watcher.rs)
    #[instrument(skip(self, path), fields(path = %path))]
    async fn watch(&self, path: &SyncPath) -> anyhow::Result<WatchHandle> {
        debug!("watch requested (returning no-op handle)");
        Ok(WatchHandle::new(|| {}))
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    /// Helper: create a [`SyncPath`] inside the given temp directory.
    fn sync_path(dir: &TempDir, name: &str) -> SyncPath {
        let p = dir.path().join(name);
        SyncPath::new(p).expect("temp dir paths are absolute")
    }

    // ------------------------------------------------------------------
    // read / write roundtrip
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_read_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "hello.txt");

        let content = b"Hello, LNXDrive!";
        fs.write_file(&path, content).await.unwrap();

        let read_back = fs.read_file(&path).await.unwrap();
        assert_eq!(read_back, content);
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "a/b/c/nested.txt");

        fs.write_file(&path, b"nested content").await.unwrap();

        let read_back = fs.read_file(&path).await.unwrap();
        assert_eq!(read_back, b"nested content");
    }

    #[tokio::test]
    async fn test_write_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "overwrite.txt");

        fs.write_file(&path, b"first").await.unwrap();
        fs.write_file(&path, b"second").await.unwrap();

        let read_back = fs.read_file(&path).await.unwrap();
        assert_eq!(read_back, b"second");
    }

    // ------------------------------------------------------------------
    // delete
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_delete_file() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "to_delete.txt");

        fs.write_file(&path, b"bye").await.unwrap();
        fs.delete_file(&path).await.unwrap();

        let state = fs.get_state(&path).await.unwrap();
        assert!(!state.exists);
    }

    #[tokio::test]
    async fn test_delete_directory() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let sub = sync_path(&dir, "subdir");
        let file_in_sub = sync_path(&dir, "subdir/file.txt");

        fs.create_directory(&sub).await.unwrap();
        fs.write_file(&file_in_sub, b"data").await.unwrap();
        fs.delete_file(&sub).await.unwrap();

        let state = fs.get_state(&sub).await.unwrap();
        assert!(!state.exists);
    }

    // ------------------------------------------------------------------
    // get_state
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_state_existing_file() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "state.txt");

        fs.write_file(&path, b"twelve bytes").await.unwrap();

        let state = fs.get_state(&path).await.unwrap();
        assert!(state.exists);
        assert!(state.is_file);
        assert_eq!(state.size, 12);
        assert!(state.modified.is_some());
        assert!(!state.is_locked);
    }

    #[tokio::test]
    async fn test_get_state_existing_directory() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let sub = sync_path(&dir, "mydir");

        fs.create_directory(&sub).await.unwrap();

        let state = fs.get_state(&sub).await.unwrap();
        assert!(state.exists);
        assert!(!state.is_file);
        assert!(!state.is_locked);
    }

    #[tokio::test]
    async fn test_get_state_not_found() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "nonexistent.txt");

        let state = fs.get_state(&path).await.unwrap();
        assert!(!state.exists);
        assert!(!state.is_file);
        assert_eq!(state.size, 0);
        assert!(state.modified.is_none());
        assert!(!state.is_locked);
    }

    // ------------------------------------------------------------------
    // compute_hash
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_compute_hash_consistent() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "hash_me.txt");

        fs.write_file(&path, b"consistent content").await.unwrap();

        let h1 = fs.compute_hash(&path).await.unwrap();
        let h2 = fs.compute_hash(&path).await.unwrap();
        assert_eq!(h1, h2);
    }

    #[tokio::test]
    async fn test_compute_hash_different_for_different_content() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let p1 = sync_path(&dir, "a.txt");
        let p2 = sync_path(&dir, "b.txt");

        fs.write_file(&p1, b"aaa").await.unwrap();
        fs.write_file(&p2, b"bbb").await.unwrap();

        let h1 = fs.compute_hash(&p1).await.unwrap();
        let h2 = fs.compute_hash(&p2).await.unwrap();
        assert_ne!(h1, h2);
    }

    #[tokio::test]
    async fn test_compute_hash_empty_file() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "empty.txt");

        fs.write_file(&path, b"").await.unwrap();

        // Should succeed without panicking and produce a valid base64 hash.
        let hash = fs.compute_hash(&path).await.unwrap();
        assert!(!hash.as_str().is_empty());
    }

    #[tokio::test]
    async fn test_compute_hash_produces_valid_base64() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "b64.txt");

        fs.write_file(&path, b"base64 test").await.unwrap();

        let hash = fs.compute_hash(&path).await.unwrap();
        // FileHash::new validates that the string is proper base64 of 20 bytes,
        // so if we got here the format is correct.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(hash.as_str())
            .unwrap();
        assert_eq!(decoded.len(), 20);
    }

    // ------------------------------------------------------------------
    // create_directory
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_directory() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = sync_path(&dir, "new/deep/dir");

        fs.create_directory(&path).await.unwrap();

        let state = fs.get_state(&path).await.unwrap();
        assert!(state.exists);
        assert!(state.is_directory());
    }

    // ------------------------------------------------------------------
    // watch (no-op stub)
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_watch_returns_handle() {
        let dir = TempDir::new().unwrap();
        let fs = LocalFileSystemAdapter::new();
        let path = SyncPath::new(PathBuf::from(dir.path())).unwrap();

        let handle = fs.watch(&path).await.unwrap();
        // Dropping the handle should not panic.
        drop(handle);
    }
}
