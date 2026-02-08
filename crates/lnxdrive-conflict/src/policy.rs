//! Policy engine for automatic conflict resolution
//!
//! Evaluates conflict rules from configuration to determine automatic resolution
//! strategies. Rules are matched using glob patterns in first-match-wins order.

use glob::Pattern;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use lnxdrive_core::domain::conflict::Resolution;

use crate::error::ConflictError;

/// A single conflict resolution rule from configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRule {
    /// Glob pattern to match file paths (e.g., "**/*.docx", "Documents/**")
    pub pattern: String,
    /// Resolution strategy to apply when the pattern matches
    pub strategy: String,
}

impl ConflictRule {
    /// Validates the rule's glob pattern and strategy
    pub fn validate(&self) -> Result<(), ConflictError> {
        Pattern::new(&self.pattern).map_err(|e| ConflictError::InvalidPattern {
            pattern: self.pattern.clone(),
            reason: e.to_string(),
        })?;

        parse_strategy(&self.strategy).ok_or_else(|| ConflictError::InvalidPattern {
            pattern: self.pattern.clone(),
            reason: format!(
                "invalid strategy '{}'; valid: keep_local, keep_remote, keep_both",
                self.strategy
            ),
        })?;

        Ok(())
    }
}

/// Engine that evaluates conflict resolution rules
pub struct PolicyEngine {
    rules: Vec<(Pattern, Resolution)>,
    default_strategy: Resolution,
}

impl PolicyEngine {
    /// Creates a PolicyEngine from the default strategy string and a list of rules
    ///
    /// Invalid rules are logged and skipped.
    pub fn new(default_strategy: &str, rules: &[ConflictRule]) -> Self {
        let default = parse_strategy(default_strategy).unwrap_or(Resolution::Manual);

        let compiled_rules: Vec<(Pattern, Resolution)> = rules
            .iter()
            .filter_map(|rule| {
                let pattern = match Pattern::new(&rule.pattern) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(
                            pattern = %rule.pattern,
                            error = %e,
                            "Skipping invalid conflict rule pattern"
                        );
                        return None;
                    }
                };
                let resolution = match parse_strategy(&rule.strategy) {
                    Some(r) => r,
                    None => {
                        tracing::warn!(
                            strategy = %rule.strategy,
                            "Skipping invalid conflict rule strategy"
                        );
                        return None;
                    }
                };
                Some((pattern, resolution))
            })
            .collect();

        debug!(
            rules_count = compiled_rules.len(),
            default = %default,
            "PolicyEngine initialized"
        );

        Self {
            rules: compiled_rules,
            default_strategy: default,
        }
    }

    /// Evaluates the policy for a given file path (relative to sync root)
    ///
    /// Uses first-match-wins: the first rule whose glob matches the path
    /// determines the resolution. If no rule matches, returns the default strategy.
    pub fn evaluate(&self, relative_path: &str) -> Resolution {
        for (pattern, resolution) in &self.rules {
            if pattern.matches(relative_path) {
                trace!(
                    path = %relative_path,
                    pattern = %pattern,
                    resolution = %resolution,
                    "Conflict rule matched"
                );
                return resolution.clone();
            }
        }

        trace!(
            path = %relative_path,
            default = %self.default_strategy,
            "No conflict rule matched, using default"
        );
        self.default_strategy.clone()
    }

    /// Returns the default resolution strategy
    pub fn default_strategy(&self) -> &Resolution {
        &self.default_strategy
    }

    /// Returns the number of compiled rules
    pub fn rules_count(&self) -> usize {
        self.rules.len()
    }
}

/// Parses a strategy string into a Resolution enum
fn parse_strategy(s: &str) -> Option<Resolution> {
    match s {
        "keep_local" => Some(Resolution::KeepLocal),
        "keep_remote" => Some(Resolution::KeepRemote),
        "keep_both" => Some(Resolution::KeepBoth),
        "manual" => Some(Resolution::Manual),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_engine_no_rules() {
        let engine = PolicyEngine::new("manual", &[]);
        assert_eq!(engine.evaluate("any/file.txt"), Resolution::Manual);
        assert_eq!(engine.rules_count(), 0);
    }

    #[test]
    fn test_policy_engine_default_strategy() {
        let engine = PolicyEngine::new("keep_local", &[]);
        assert_eq!(engine.evaluate("any/file.txt"), Resolution::KeepLocal);
    }

    #[test]
    fn test_policy_engine_first_match_wins() {
        let rules = vec![
            ConflictRule {
                pattern: "**/*.docx".to_string(),
                strategy: "keep_both".to_string(),
            },
            ConflictRule {
                pattern: "**/*".to_string(),
                strategy: "keep_remote".to_string(),
            },
        ];

        let engine = PolicyEngine::new("manual", &rules);

        assert_eq!(
            engine.evaluate("Documents/report.docx"),
            Resolution::KeepBoth
        );
        assert_eq!(
            engine.evaluate("Documents/report.pdf"),
            Resolution::KeepRemote
        );
    }

    #[test]
    fn test_policy_engine_glob_patterns() {
        let rules = vec![
            ConflictRule {
                pattern: "*.tmp".to_string(),
                strategy: "keep_remote".to_string(),
            },
            ConflictRule {
                pattern: "Documents/**/*.xlsx".to_string(),
                strategy: "keep_both".to_string(),
            },
        ];

        let engine = PolicyEngine::new("manual", &rules);

        assert_eq!(engine.evaluate("test.tmp"), Resolution::KeepRemote);
        assert_eq!(
            engine.evaluate("Documents/Finance/budget.xlsx"),
            Resolution::KeepBoth
        );
        assert_eq!(engine.evaluate("other.txt"), Resolution::Manual);
    }

    #[test]
    fn test_policy_engine_invalid_rules_skipped() {
        let rules = vec![
            ConflictRule {
                pattern: "[invalid".to_string(),
                strategy: "keep_local".to_string(),
            },
            ConflictRule {
                pattern: "**/*.txt".to_string(),
                strategy: "invalid_strategy".to_string(),
            },
            ConflictRule {
                pattern: "**/*.rs".to_string(),
                strategy: "keep_local".to_string(),
            },
        ];

        let engine = PolicyEngine::new("manual", &rules);
        assert_eq!(engine.rules_count(), 1);
        assert_eq!(engine.evaluate("test.rs"), Resolution::KeepLocal);
    }

    #[test]
    fn test_policy_engine_invalid_default() {
        let engine = PolicyEngine::new("garbage", &[]);
        assert_eq!(*engine.default_strategy(), Resolution::Manual);
    }

    #[test]
    fn test_conflict_rule_validate_valid() {
        let rule = ConflictRule {
            pattern: "**/*.docx".to_string(),
            strategy: "keep_both".to_string(),
        };
        assert!(rule.validate().is_ok());
    }

    #[test]
    fn test_conflict_rule_validate_invalid_pattern() {
        let rule = ConflictRule {
            pattern: "[invalid".to_string(),
            strategy: "keep_both".to_string(),
        };
        assert!(matches!(
            rule.validate(),
            Err(ConflictError::InvalidPattern { .. })
        ));
    }

    #[test]
    fn test_conflict_rule_validate_invalid_strategy() {
        let rule = ConflictRule {
            pattern: "**/*.txt".to_string(),
            strategy: "yolo".to_string(),
        };
        assert!(matches!(
            rule.validate(),
            Err(ConflictError::InvalidPattern { .. })
        ));
    }

    #[test]
    fn test_parse_strategy() {
        assert_eq!(parse_strategy("keep_local"), Some(Resolution::KeepLocal));
        assert_eq!(parse_strategy("keep_remote"), Some(Resolution::KeepRemote));
        assert_eq!(parse_strategy("keep_both"), Some(Resolution::KeepBoth));
        assert_eq!(parse_strategy("manual"), Some(Resolution::Manual));
        assert_eq!(parse_strategy("unknown"), None);
    }
}
