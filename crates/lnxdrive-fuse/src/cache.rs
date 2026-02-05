//! File content cache for storing hydrated file data.
//!
//! Uses a hash-based directory structure for efficient storage and lookup.

use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use lnxdrive_core::domain::newtypes::RemoteId;
use sha2::{Digest, Sha256};

use crate::error::FuseError;

/// Manages cached file content on disk.
///
/// Content is stored in a hash-based directory structure:
/// `{cache_dir}/content/{first_2_chars_of_hash}/{rest_of_hash}`
pub struct ContentCache {
    #[allow(dead_code)]
    cache_dir: PathBuf,
    content_dir: PathBuf,
}

impl ContentCache {
    /// Create a new ContentCache, creating the content directory if needed.
    pub fn new(cache_dir: PathBuf) -> std::io::Result<Self> {
        let content_dir = cache_dir.join("content");
        fs::create_dir_all(&content_dir)?;
        Ok(Self {
            cache_dir,
            content_dir,
        })
    }

    /// Compute the cache path for a remote ID using SHA-256 hash.
    pub fn cache_path(&self, remote_id: &RemoteId) -> PathBuf {
        let hash = Self::hash_remote_id(remote_id);
        let (prefix, rest) = hash.split_at(2);
        self.content_dir.join(prefix).join(rest)
    }

    /// Get the path for a partial (in-progress) download.
    pub fn partial_path(&self, remote_id: &RemoteId) -> PathBuf {
        let mut path = self.cache_path(remote_id);
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        path.set_file_name(format!("{}.partial", filename));
        path
    }

    /// Store data in the cache.
    pub fn store(&self, remote_id: &RemoteId, data: &[u8]) -> Result<PathBuf, FuseError> {
        let path = self.cache_path(remote_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(&path)?;
        file.write_all(data)?;
        Ok(path)
    }

    /// Read bytes from cached file at offset.
    pub fn read(&self, remote_id: &RemoteId, offset: u64, size: u32) -> Result<Vec<u8>, FuseError> {
        let path = self.cache_path(remote_id);
        let mut file = File::open(&path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0u8; size as usize];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    /// Check if content exists in cache.
    pub fn exists(&self, remote_id: &RemoteId) -> bool {
        self.cache_path(remote_id).exists()
    }

    /// Remove cached content.
    pub fn remove(&self, remote_id: &RemoteId) -> Result<(), FuseError> {
        let path = self.cache_path(remote_id);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        // Also try to remove partial file if it exists
        let partial = self.partial_path(remote_id);
        if partial.exists() {
            let _ = fs::remove_file(&partial);
        }
        Ok(())
    }

    /// Write data to a cached file at the specified offset.
    ///
    /// Opens the cache file (creating if needed), seeks to offset, and writes data.
    /// Returns the number of bytes written.
    ///
    /// # Arguments
    /// * `remote_id` - The remote ID to identify the cache file
    /// * `offset` - Byte offset to start writing at
    /// * `data` - Data to write
    ///
    /// # Returns
    /// Number of bytes written
    pub fn write_at(
        &self,
        remote_id: &RemoteId,
        offset: u64,
        data: &[u8],
    ) -> Result<u32, FuseError> {
        let path = self.cache_path(remote_id);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open file with read/write, create if doesn't exist
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        // Seek to offset
        file.seek(SeekFrom::Start(offset))?;

        // Write data
        file.write_all(data)?;

        Ok(data.len() as u32)
    }

    /// Calculate total disk usage of the cache.
    pub fn disk_usage(&self) -> Result<u64, FuseError> {
        let mut total = 0u64;
        if self.content_dir.exists() {
            for entry in fs::read_dir(&self.content_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    for file in fs::read_dir(entry.path())? {
                        let file = file?;
                        if file.file_type()?.is_file() {
                            total += file.metadata()?.len();
                        }
                    }
                }
            }
        }
        Ok(total)
    }

    fn hash_remote_id(remote_id: &RemoteId) -> String {
        let mut hasher = Sha256::new();
        hasher.update(remote_id.as_str().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_cache_path_produces_correct_hash_layout() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("test-file-id-123".to_string()).expect("Failed to create RemoteId");
        let cache_path = cache.cache_path(&remote_id);

        // Verify the hash is computed correctly
        let expected_hash = {
            let mut hasher = Sha256::new();
            hasher.update(remote_id.as_str().as_bytes());
            format!("{:x}", hasher.finalize())
        };

        // Verify 2-char prefix directory structure
        let (prefix, rest) = expected_hash.split_at(2);
        let expected_path = temp_dir.path().join("content").join(prefix).join(rest);

        assert_eq!(cache_path, expected_path);
        assert!(cache_path.to_string_lossy().contains(&prefix.to_string()));
    }

    #[test]
    fn test_store_and_read_roundtrip() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("roundtrip-test-id".to_string()).expect("Failed to create RemoteId");
        let test_data = b"Hello, LNXDrive! This is test content.";

        // Store data
        let stored_path = cache
            .store(&remote_id, test_data)
            .expect("Failed to store data");
        assert!(stored_path.exists());

        // Read full content
        let read_data = cache
            .read(&remote_id, 0, test_data.len() as u32)
            .expect("Failed to read data");
        assert_eq!(read_data, test_data);

        // Read partial content (offset and size)
        let partial_data = cache
            .read(&remote_id, 7, 9)
            .expect("Failed to read partial");
        assert_eq!(partial_data, &test_data[7..16]);
    }

    #[test]
    fn test_exists_returns_correct_bool() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("exists-test-id".to_string()).expect("Failed to create RemoteId");

        // Should not exist initially
        assert!(!cache.exists(&remote_id));

        // Store data
        cache
            .store(&remote_id, b"test content")
            .expect("Failed to store data");

        // Should exist now
        assert!(cache.exists(&remote_id));
    }

    #[test]
    fn test_remove_deletes_file() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("remove-test-id".to_string()).expect("Failed to create RemoteId");
        let test_data = b"data to be removed";

        // Store data
        cache
            .store(&remote_id, test_data)
            .expect("Failed to store data");
        assert!(cache.exists(&remote_id));

        // Remove data
        cache.remove(&remote_id).expect("Failed to remove data");
        assert!(!cache.exists(&remote_id));

        // Removing non-existent file should not error
        cache
            .remove(&remote_id)
            .expect("Remove should be idempotent");
    }

    #[test]
    fn test_disk_usage_computes_correctly() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        // Initially empty
        let initial_usage = cache.disk_usage().expect("Failed to get disk usage");
        assert_eq!(initial_usage, 0);

        // Store some files
        let file1_data = b"File 1 content";
        let file2_data = b"File 2 has more content here";
        let file3_data = b"Third file";

        cache
            .store(
                &RemoteId::new("file1".to_string()).expect("Failed to create RemoteId"),
                file1_data,
            )
            .expect("Failed to store file1");
        cache
            .store(
                &RemoteId::new("file2".to_string()).expect("Failed to create RemoteId"),
                file2_data,
            )
            .expect("Failed to store file2");
        cache
            .store(
                &RemoteId::new("file3".to_string()).expect("Failed to create RemoteId"),
                file3_data,
            )
            .expect("Failed to store file3");

        // Check total usage
        let total_usage = cache.disk_usage().expect("Failed to get disk usage");
        let expected_size = file1_data.len() + file2_data.len() + file3_data.len();
        assert_eq!(total_usage, expected_size as u64);

        // Remove one file and verify usage decreases
        cache
            .remove(&RemoteId::new("file2".to_string()).expect("Failed to create RemoteId"))
            .expect("Failed to remove file2");
        let new_usage = cache.disk_usage().expect("Failed to get disk usage");
        let expected_new_size = file1_data.len() + file3_data.len();
        assert_eq!(new_usage, expected_new_size as u64);
    }

    #[test]
    fn test_partial_path_has_partial_suffix() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("partial-test-id".to_string()).expect("Failed to create RemoteId");
        let cache_path = cache.cache_path(&remote_id);
        let partial_path = cache.partial_path(&remote_id);

        // Verify partial path has .partial suffix
        assert!(partial_path.to_string_lossy().ends_with(".partial"));

        // Verify the base name is the same except for .partial suffix
        let cache_filename = cache_path.file_name().unwrap().to_string_lossy();
        let partial_filename = partial_path.file_name().unwrap().to_string_lossy();
        assert_eq!(partial_filename, format!("{}.partial", cache_filename));

        // Verify they share the same parent directory
        assert_eq!(cache_path.parent(), partial_path.parent());
    }

    #[test]
    fn test_remove_also_removes_partial_file() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("partial-removal-test".to_string()).expect("Failed to create RemoteId");
        let partial_path = cache.partial_path(&remote_id);

        // Manually create a partial file
        if let Some(parent) = partial_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dir");
        }
        let mut partial_file = File::create(&partial_path).expect("Failed to create partial file");
        partial_file
            .write_all(b"partial content")
            .expect("Failed to write partial");
        drop(partial_file);

        assert!(partial_path.exists());

        // Remove should delete the partial file
        cache.remove(&remote_id).expect("Failed to remove");
        assert!(!partial_path.exists());
    }

    #[test]
    fn test_read_empty_file() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("empty-file-test".to_string()).expect("Failed to create RemoteId");

        // Store empty file
        cache
            .store(&remote_id, b"")
            .expect("Failed to store empty file");

        // Read should return empty vec
        let data = cache.read(&remote_id, 0, 0).expect("Failed to read");
        assert_eq!(data, b"");
    }

    #[test]
    fn test_read_beyond_file_size() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("read-beyond-test".to_string()).expect("Failed to create RemoteId");
        let test_data = b"short";

        cache.store(&remote_id, test_data).expect("Failed to store");

        // Request more bytes than available
        let data = cache.read(&remote_id, 0, 100).expect("Failed to read");

        // Should only return available bytes
        assert_eq!(data, test_data);
    }

    #[test]
    fn test_write_at_creates_file_if_not_exists() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("write-new-file".to_string()).expect("Failed to create RemoteId");
        let test_data = b"Hello, World!";

        // File should not exist initially
        assert!(!cache.exists(&remote_id));

        // Write to a new file
        let bytes_written = cache
            .write_at(&remote_id, 0, test_data)
            .expect("Failed to write_at");

        assert_eq!(bytes_written, test_data.len() as u32);
        assert!(cache.exists(&remote_id));

        // Verify content
        let read_data = cache
            .read(&remote_id, 0, test_data.len() as u32)
            .expect("Failed to read");
        assert_eq!(read_data, test_data);
    }

    #[test]
    fn test_write_at_writes_at_offset() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("write-offset-test".to_string()).expect("Failed to create RemoteId");

        // Create initial file with content
        let initial_data = b"Hello, World!";
        cache
            .store(&remote_id, initial_data)
            .expect("Failed to store initial data");

        // Overwrite part of the file at offset 7
        let replacement = b"Rust!!";
        let bytes_written = cache
            .write_at(&remote_id, 7, replacement)
            .expect("Failed to write_at");

        assert_eq!(bytes_written, replacement.len() as u32);

        // Verify the result: "Hello, Rust!!"
        let read_data = cache.read(&remote_id, 0, 13).expect("Failed to read");
        assert_eq!(read_data, b"Hello, Rust!!");
    }

    #[test]
    fn test_write_at_extends_file() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache = ContentCache::new(temp_dir.path().to_path_buf())
            .expect("Failed to create ContentCache");

        let remote_id =
            RemoteId::new("write-extend-test".to_string()).expect("Failed to create RemoteId");

        // Create initial file with content
        let initial_data = b"Hello";
        cache
            .store(&remote_id, initial_data)
            .expect("Failed to store initial data");

        // Verify initial size
        let initial_read = cache.read(&remote_id, 0, 100).expect("Failed to read");
        assert_eq!(initial_read.len(), 5);

        // Write beyond the current file size
        let extension = b", World!";
        let bytes_written = cache
            .write_at(&remote_id, 5, extension)
            .expect("Failed to write_at");

        assert_eq!(bytes_written, extension.len() as u32);

        // Verify the extended content
        let read_data = cache.read(&remote_id, 0, 100).expect("Failed to read");
        assert_eq!(read_data, b"Hello, World!");
    }
}
