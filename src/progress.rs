//! Build progress reporting using indicatif spinners.
//!
//! Shows per-step progress during the build pipeline. Only displays
//! spinners when stdout is a TTY; otherwise produces no output so
//! piped / CI output stays clean.

use std::time::Instant;

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

/// Tracks build progress and displays an animated spinner for each pipeline step.
pub struct BuildProgress {
    spinner: Option<ProgressBar>,
    step_start: Instant,
    is_tty: bool,
}

impl BuildProgress {
    /// Create a new progress reporter. Only shows spinners if stdout is a TTY.
    pub fn new() -> Self {
        let is_tty = console::Term::stdout().is_term();
        Self {
            spinner: None,
            step_start: Instant::now(),
            is_tty,
        }
    }

    /// Start a new step. Finishes the previous step's spinner (if any) with a checkmark.
    pub fn step(&mut self, label: &str) {
        self.finish_current();
        if !self.is_tty {
            return;
        }
        self.step_start = Instant::now();
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("  {spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"]),
        );
        pb.set_message(label.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.spinner = Some(pb);
    }

    /// Finish the current step's spinner with timing info.
    fn finish_current(&mut self) {
        if let Some(pb) = self.spinner.take() {
            let elapsed = self.step_start.elapsed();
            let ms = elapsed.as_secs_f64() * 1000.0;
            let timing = if ms >= 1000.0 {
                format!("{:.1}s", ms / 1000.0)
            } else if ms >= 1.0 {
                format!("{:.0}ms", ms)
            } else {
                "<1ms".to_string()
            };
            let msg = pb.message();
            pb.finish_with_message(format!(
                "{} {} {}",
                style("✓").green(),
                msg,
                style(format!("({timing})")).dim()
            ));
        }
    }

    /// Finish the final step.
    pub fn done(mut self) {
        self.finish_current();
    }
}

impl Default for BuildProgress {
    fn default() -> Self {
        Self::new()
    }
}
