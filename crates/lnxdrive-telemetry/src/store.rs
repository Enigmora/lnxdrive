//! Local report storage
//!
//! Manages crash and error report files in `~/.local/share/lnxdrive/reports/`.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Entry in the local report store
#[derive(Debug, Clone)]
pub struct ReportEntry {
    pub id: String,
    pub report_type: String,
    pub date: String,
    pub size_bytes: u64,
    pub path: PathBuf,
}

/// Manages the local directory of crash/error report files.
pub struct LocalReportStore {
    reports_dir: PathBuf,
}

impl LocalReportStore {
    /// Creates a new store pointing at `reports_dir`.
    pub fn new(reports_dir: PathBuf) -> Self {
        Self { reports_dir }
    }

    /// Returns the default reports directory.
    pub fn default_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("lnxdrive")
            .join("reports")
    }

    /// List all report files in the store.
    pub fn list(&self) -> anyhow::Result<Vec<ReportEntry>> {
        if !self.reports_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&self.reports_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "json") {
                let filename = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let (report_type, date, id) = parse_report_filename(&filename);
                let metadata = entry.metadata()?;

                entries.push(ReportEntry {
                    id,
                    report_type,
                    date,
                    size_bytes: metadata.len(),
                    path,
                });
            }
        }

        entries.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(entries)
    }

    /// Read a report by its ID (filename stem match).
    pub fn read(&self, id: &str) -> anyhow::Result<Option<Value>> {
        let entries = self.list()?;
        for entry in entries {
            if entry.id == id || entry.path.file_stem().unwrap_or_default().to_string_lossy().contains(id) {
                let content = std::fs::read_to_string(&entry.path)?;
                let value: Value = serde_json::from_str(&content)?;
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Delete a report by its ID.
    pub fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let entries = self.list()?;
        for entry in entries {
            if entry.id == id || entry.path.file_stem().unwrap_or_default().to_string_lossy().contains(id) {
                std::fs::remove_file(&entry.path)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Delete all reports.
    pub fn delete_all(&self) -> anyhow::Result<u32> {
        let entries = self.list()?;
        let mut count = 0;
        for entry in entries {
            if std::fs::remove_file(&entry.path).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Returns the reports directory path.
    pub fn reports_dir(&self) -> &Path {
        &self.reports_dir
    }
}

/// Parse a report filename like `crash-20260207-a1b2c3d4` into (type, date, id).
fn parse_report_filename(stem: &str) -> (String, String, String) {
    let parts: Vec<&str> = stem.splitn(3, '-').collect();
    match parts.len() {
        3 => (
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ),
        2 => (parts[0].to_string(), parts[1].to_string(), stem.to_string()),
        _ => ("unknown".to_string(), String::new(), stem.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalReportStore::new(dir.path().to_path_buf());
        let entries = store.list().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_nonexistent_dir() {
        let store = LocalReportStore::new(PathBuf::from("/nonexistent/path"));
        let entries = store.list().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_store_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalReportStore::new(dir.path().to_path_buf());

        // Write a fake report
        let report = serde_json::json!({
            "id": "test123",
            "message": "test crash"
        });
        std::fs::write(
            dir.path().join("crash-20260207-test123.json"),
            serde_json::to_string(&report).unwrap(),
        )
        .unwrap();

        // List
        let entries = store.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].report_type, "crash");

        // Read
        let content = store.read("test123").unwrap();
        assert!(content.is_some());
        assert_eq!(content.unwrap()["id"], "test123");

        // Delete
        assert!(store.delete("test123").unwrap());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn test_delete_all() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalReportStore::new(dir.path().to_path_buf());

        // Write two fake reports
        for i in 0..3 {
            let path = dir.path().join(format!("crash-20260207-id{i}.json"));
            std::fs::write(&path, "{}").unwrap();
        }

        assert_eq!(store.list().unwrap().len(), 3);
        let count = store.delete_all().unwrap();
        assert_eq!(count, 3);
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn test_parse_report_filename() {
        let (t, d, id) = parse_report_filename("crash-20260207-abc12345");
        assert_eq!(t, "crash");
        assert_eq!(d, "20260207");
        assert_eq!(id, "abc12345");
    }
}
