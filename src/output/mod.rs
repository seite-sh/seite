pub mod human;
pub mod json;

use serde::Serialize;

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
