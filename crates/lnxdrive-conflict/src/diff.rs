//! Diff tool launcher for visual comparison
//!
//! Detects and launches external diff tools (meld, vimdiff, diff) for
//! comparing local and remote file versions during conflict resolution.

use std::path::Path;

use tracing::{debug, info};

use crate::error::ConflictError;

/// Supported diff tools in order of preference
const DIFF_TOOLS: &[(&str, &[&str])] = &[
    ("meld", &[]),
    ("kdiff3", &[]),
    ("vimdiff", &[]),
    ("diff", &["--color=auto", "-u"]),
];

/// Launches external diff tools for conflict comparison
pub struct DiffToolLauncher;

impl DiffToolLauncher {
    /// Detect the best available diff tool on the system
    ///
    /// Checks in order: user override → meld → kdiff3 → vimdiff → diff
    pub fn detect(override_tool: Option<&str>) -> Result<String, ConflictError> {
        if let Some(tool) = override_tool {
            if Self::is_available(tool) {
                return Ok(tool.to_string());
            }
            return Err(ConflictError::DiffToolNotFound(format!(
                "Configured diff tool '{}' not found in PATH",
                tool
            )));
        }

        for (tool, _) in DIFF_TOOLS {
            if Self::is_available(tool) {
                debug!(tool, "Detected diff tool");
                return Ok(tool.to_string());
            }
        }

        Err(ConflictError::DiffToolNotFound(
            "No diff tool found. Install meld, kdiff3, or vimdiff".to_string(),
        ))
    }

    /// Launch the diff tool comparing two files
    ///
    /// For GUI tools (meld, kdiff3), spawns in the background.
    /// For terminal tools (vimdiff, diff), runs in the foreground.
    pub fn launch(
        tool: &str,
        local_path: &Path,
        remote_path: &Path,
    ) -> Result<(), ConflictError> {
        info!(
            tool,
            local = %local_path.display(),
            remote = %remote_path.display(),
            "Launching diff tool"
        );

        let extra_args = DIFF_TOOLS
            .iter()
            .find(|(name, _)| *name == tool)
            .map(|(_, args)| args.to_vec())
            .unwrap_or_default();

        let mut cmd = std::process::Command::new(tool);

        for arg in &extra_args {
            cmd.arg(arg);
        }

        cmd.arg(local_path).arg(remote_path);

        // GUI tools run in background, terminal tools in foreground
        if Self::is_gui_tool(tool) {
            cmd.spawn().map_err(|e| {
                ConflictError::DiffToolNotFound(format!("Failed to launch {tool}: {e}"))
            })?;
        } else {
            cmd.status().map_err(|e| {
                ConflictError::DiffToolNotFound(format!("Failed to run {tool}: {e}"))
            })?;
        }

        Ok(())
    }

    /// Check if a tool is available in PATH
    fn is_available(tool: &str) -> bool {
        std::process::Command::new("which")
            .arg(tool)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Whether a tool is a GUI application (runs in background)
    fn is_gui_tool(tool: &str) -> bool {
        matches!(tool, "meld" | "kdiff3" | "kompare" | "diffuse")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_fallback_to_diff() {
        // diff should always be available on Linux
        let result = DiffToolLauncher::detect(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_with_invalid_override() {
        let result = DiffToolLauncher::detect(Some("nonexistent_tool_xyz"));
        assert!(matches!(result, Err(ConflictError::DiffToolNotFound(_))));
    }

    #[test]
    fn test_is_gui_tool() {
        assert!(DiffToolLauncher::is_gui_tool("meld"));
        assert!(DiffToolLauncher::is_gui_tool("kdiff3"));
        assert!(!DiffToolLauncher::is_gui_tool("vimdiff"));
        assert!(!DiffToolLauncher::is_gui_tool("diff"));
    }

    #[test]
    fn test_is_available_diff() {
        // diff should always be available
        assert!(DiffToolLauncher::is_available("diff"));
    }

    #[test]
    fn test_is_available_nonexistent() {
        assert!(!DiffToolLauncher::is_available("nonexistent_tool_xyz_123"));
    }
}
