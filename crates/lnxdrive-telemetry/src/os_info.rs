//! Operating system information collector
//!
//! Gathers non-identifying system information for crash/error reports.
//! Never includes hostname or username.

use serde::{Deserialize, Serialize};

/// Non-identifying operating system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub os: String,
    pub kernel: String,
    pub desktop: String,
    pub arch: String,
}

impl OsInfo {
    /// Collect OS information from the current system.
    pub fn collect() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            kernel: read_kernel_version(),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

fn read_kernel_version() -> String {
    std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_os_info() {
        let info = OsInfo::collect();
        assert_eq!(info.os, "linux");
        assert!(!info.arch.is_empty());
    }

    #[test]
    fn test_os_info_serialization() {
        let info = OsInfo::collect();
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: OsInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.os, info.os);
        assert_eq!(deserialized.arch, info.arch);
    }
}
