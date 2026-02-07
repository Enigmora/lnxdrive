//! Conflict naming for keep-both resolution
//!
//! Generates unique file names for conflict copies, following the pattern:
//! `filename (conflicted copy YYYY-MM-DD XXXXXXXX).ext`

use chrono::Utc;
use uuid::Uuid;

/// Generates unique conflict file names
pub struct ConflictNamer;

impl ConflictNamer {
    /// Generates a conflict copy filename
    ///
    /// Given "report.docx", produces something like:
    /// "report (conflicted copy 2026-02-07 a1b2c3d4).docx"
    pub fn generate(original_name: &str) -> String {
        let timestamp = Utc::now().format("%Y-%m-%d");
        let short_uuid = &Uuid::new_v4().to_string()[..8];

        if let Some(dot_pos) = original_name.rfind('.') {
            let stem = &original_name[..dot_pos];
            let ext = &original_name[dot_pos..];
            format!("{stem} (conflicted copy {timestamp} {short_uuid}){ext}")
        } else {
            format!("{original_name} (conflicted copy {timestamp} {short_uuid})")
        }
    }

    /// Verifies the generated name doesn't collide with existing names
    ///
    /// If the name already exists, appends an incrementing suffix.
    pub fn generate_unique<F>(original_name: &str, mut exists: F) -> String
    where
        F: FnMut(&str) -> bool,
    {
        let candidate = Self::generate(original_name);
        if !exists(&candidate) {
            return candidate;
        }

        // Extremely unlikely with UUID, but handle it
        for i in 2..=99 {
            let numbered = if let Some(dot_pos) = candidate.rfind('.') {
                let stem = &candidate[..dot_pos];
                let ext = &candidate[dot_pos..];
                format!("{stem} {i}{ext}")
            } else {
                format!("{candidate} {i}")
            };

            if !exists(&numbered) {
                return numbered;
            }
        }

        // Last resort: full UUID
        let full_uuid = Uuid::new_v4();
        format!("{original_name}.conflict-{full_uuid}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_with_extension() {
        let name = ConflictNamer::generate("report.docx");
        assert!(name.starts_with("report (conflicted copy "));
        assert!(name.ends_with(").docx"));
        assert!(name.contains("20")); // year prefix
    }

    #[test]
    fn test_generate_without_extension() {
        let name = ConflictNamer::generate("Makefile");
        assert!(name.starts_with("Makefile (conflicted copy "));
        assert!(name.ends_with(')'));
    }

    #[test]
    fn test_generate_with_multiple_dots() {
        let name = ConflictNamer::generate("archive.tar.gz");
        assert!(name.ends_with(").gz"));
        assert!(name.contains("archive.tar (conflicted copy"));
    }

    #[test]
    fn test_generate_unique_no_collision() {
        let name = ConflictNamer::generate_unique("test.txt", |_| false);
        assert!(name.contains("conflicted copy"));
    }

    #[test]
    fn test_generate_unique_with_collision() {
        let mut call_count = 0;
        let name = ConflictNamer::generate_unique("test.txt", |_| {
            call_count += 1;
            call_count <= 1 // first candidate collides
        });
        assert!(name.contains("conflicted copy"));
    }

    #[test]
    fn test_uniqueness() {
        let name1 = ConflictNamer::generate("test.txt");
        let name2 = ConflictNamer::generate("test.txt");
        // UUIDs ensure different names
        assert_ne!(name1, name2);
    }
}
