//! Text anonymization for telemetry reports
//!
//! Replaces personally identifiable information (paths, usernames, filenames)
//! with generic placeholders before reports are stored or sent.

use lnxdrive_core::config::AnonymizeConfig;

/// Anonymizes text based on the provided configuration.
pub struct Anonymizer {
    strip_paths: bool,
    strip_usernames: bool,
    strip_filenames: bool,
    home_dir: String,
    username: String,
}

impl Anonymizer {
    /// Creates a new `Anonymizer` from configuration.
    pub fn new(config: &AnonymizeConfig) -> Self {
        let home_dir = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default();

        Self {
            strip_paths: config.strip_paths,
            strip_usernames: config.strip_usernames,
            strip_filenames: config.strip_filenames,
            home_dir,
            username,
        }
    }

    /// Anonymize the given text by applying configured replacements.
    pub fn anonymize(&self, text: &str) -> String {
        let mut result = text.to_string();

        if self.strip_paths && !self.home_dir.is_empty() {
            result = result.replace(&self.home_dir, "<HOME>");
        }

        if self.strip_usernames && !self.username.is_empty() {
            result = result.replace(&self.username, "<USER>");
        }

        if self.strip_filenames {
            result = anonymize_filenames(&result);
        }

        result
    }
}

/// Replace common file path patterns with anonymized versions.
///
/// Targets patterns like `/path/to/file.ext` in the middle of text,
/// preserving the directory structure but replacing the filename.
fn anonymize_filenames(text: &str) -> String {
    // Simple heuristic: replace the basename in paths that look like
    // /some/path/filename.ext
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        result.push(ch);

        // Look for a path separator followed by a filename-like pattern
        if ch == '/' {
            // Collect potential filename characters
            let mut segment = String::new();
            while let Some(&next) = chars.peek() {
                if next == '/' || next == ' ' || next == '\n' || next == ':' || next == '"' {
                    break;
                }
                segment.push(chars.next().unwrap());
            }

            // If segment contains a dot and looks like a file (has extension),
            // and the next char is a boundary, replace it
            if let Some(dot_pos) = segment.rfind('.') {
                if dot_pos > 0 && dot_pos < segment.len() - 1 {
                    let ext = &segment[dot_pos..];
                    result.push_str(&format!("<FILE>{ext}"));
                    continue;
                }
            }

            result.push_str(&segment);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(paths: bool, usernames: bool, filenames: bool) -> AnonymizeConfig {
        AnonymizeConfig {
            strip_paths: paths,
            strip_usernames: usernames,
            strip_filenames: filenames,
        }
    }

    #[test]
    fn test_strip_home_dir() {
        let config = make_config(true, false, false);
        let anon = Anonymizer::new(&config);

        if !anon.home_dir.is_empty() {
            let text = format!("Error at {}/Documents/secret.txt", anon.home_dir);
            let result = anon.anonymize(&text);
            assert!(result.contains("<HOME>"));
            assert!(!result.contains(&anon.home_dir));
        }
    }

    #[test]
    fn test_strip_username() {
        let config = make_config(false, true, false);
        let anon = Anonymizer::new(&config);

        if !anon.username.is_empty() {
            let text = format!("User {} encountered error", anon.username);
            let result = anon.anonymize(&text);
            assert!(result.contains("<USER>"));
            assert!(!result.contains(&anon.username));
        }
    }

    #[test]
    fn test_strip_filenames() {
        let config = make_config(false, false, true);
        let anon = Anonymizer::new(&config);

        let text = "Error reading /path/to/secret.docx";
        let result = anon.anonymize(text);
        assert!(result.contains("<FILE>.docx"));
        assert!(!result.contains("secret"));
    }

    #[test]
    fn test_no_anonymization_when_disabled() {
        let config = make_config(false, false, false);
        let anon = Anonymizer::new(&config);

        let text = "plain text with no PII";
        assert_eq!(anon.anonymize(text), text);
    }

    #[test]
    fn test_combined_anonymization() {
        let config = make_config(true, true, false);
        let anon = Anonymizer::new(&config);

        // Even with both enabled, the function should not panic
        let text = "some diagnostic text";
        let _result = anon.anonymize(text);
    }
}
