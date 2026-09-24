pub mod human;
pub mod json;

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

/// Process-global output flags, set once in `main` from the global CLI flags
/// so deeply nested code can adapt without threading arguments everywhere.
static JSON_MODE: AtomicBool = AtomicBool::new(false);
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Enable or disable machine-readable (`--json`) output mode.
pub fn set_json_mode(enabled: bool) {
    JSON_MODE.store(enabled, Ordering::Relaxed);
}

/// Whether the global `--json` flag is active. In JSON mode, all human-readable
/// output is routed to stderr so stdout carries exactly one JSON document.
pub fn is_json() -> bool {
    JSON_MODE.load(Ordering::Relaxed)
}

/// Enable or disable verbose (`--verbose`) output.
pub fn set_verbose(enabled: bool) {
    VERBOSE.store(enabled, Ordering::Relaxed);
}

/// Whether the global `--verbose` flag is active.
pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Trait for command outputs that can be rendered in both human and JSON formats.
pub trait CommandOutput: Serialize {
    fn human_display(&self) -> String;
}

/// Print a command output in the requested format.
pub fn print_output<T: CommandOutput>(output: &T, format: OutputFormat) {
    match format {
        OutputFormat::Human => println!("{}", output.human_display()),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(output).expect("failed to serialize output")
            );
        }
    }
}

/// Simple message output for commands that just need to report a string.
#[derive(Debug, Serialize)]
pub struct MessageOutput {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl CommandOutput for MessageOutput {
    fn human_display(&self) -> String {
        match &self.detail {
            Some(detail) => format!("{}\n{}", self.message, detail),
            None => self.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_output_without_detail() {
        let out = MessageOutput {
            message: "Done".into(),
            detail: None,
        };
        assert_eq!(out.human_display(), "Done");
    }

    #[test]
    fn test_message_output_with_detail() {
        let out = MessageOutput {
            message: "Built site".into(),
            detail: Some("12 pages in 0.5s".into()),
        };
        assert_eq!(out.human_display(), "Built site\n12 pages in 0.5s");
    }

    #[test]
    fn test_message_output_serialization() {
        let out = MessageOutput {
            message: "ok".into(),
            detail: None,
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["message"], "ok");
        assert!(json.get("detail").is_none()); // skip_serializing_if
    }

    #[test]
    fn test_message_output_serialization_with_detail() {
        let out = MessageOutput {
            message: "ok".into(),
            detail: Some("extra".into()),
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["message"], "ok");
        assert_eq!(json["detail"], "extra");
    }

    #[test]
    fn test_output_format_debug() {
        // Ensure OutputFormat derives Debug
        let f = OutputFormat::Human;
        let dbg = format!("{:?}", f);
        assert_eq!(dbg, "Human");
    }

    #[test]
    fn test_output_format_clone_copy() {
        let f = OutputFormat::Json;
        let f2 = f; // Copy
        let f3 = f; // Copy (same as clone for Copy types)
        assert!(matches!(f2, OutputFormat::Json));
        assert!(matches!(f3, OutputFormat::Json));
    }
}
