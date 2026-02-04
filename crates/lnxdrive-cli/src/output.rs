/// Output format selector
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Trait for formatting CLI output
pub trait OutputFormatter {
    fn success(&self, message: &str);
    fn error(&self, message: &str);
    fn warn(&self, message: &str);
    fn info(&self, message: &str);
    fn print_json(&self, value: &serde_json::Value);
}

/// Human-readable output formatter with checkmarks and indentation
pub struct HumanFormatter;

impl OutputFormatter for HumanFormatter {
    fn success(&self, message: &str) {
        println!("\u{2713} {}", message);
    }
    fn error(&self, message: &str) {
        eprintln!("\u{2717} Error: {}", message);
    }
    fn warn(&self, message: &str) {
        eprintln!("\u{26a0} Warning: {}", message);
    }
    fn info(&self, message: &str) {
        println!("  {}", message);
    }
    fn print_json(&self, _value: &serde_json::Value) {
        // Human formatter doesn't print JSON
    }
}

/// JSON output formatter
pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn success(&self, message: &str) {
        println!(
            "{}",
            serde_json::json!({"success": true, "message": message})
        );
    }
    fn error(&self, message: &str) {
        eprintln!(
            "{}",
            serde_json::json!({"success": false, "error": message})
        );
    }
    fn warn(&self, message: &str) {
        eprintln!(
            "{}",
            serde_json::json!({"level": "warning", "message": message})
        );
    }
    fn info(&self, _message: &str) {}
    fn print_json(&self, value: &serde_json::Value) {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
    }
}

pub fn get_formatter(json: bool) -> Box<dyn OutputFormatter> {
    if json {
        Box::new(JsonFormatter)
    } else {
        Box::new(HumanFormatter)
    }
}
