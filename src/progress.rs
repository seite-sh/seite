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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn finish_current(&mut self) {
        if let Some(pb) = self.spinner.take() {
            let elapsed = self.step_start.elapsed();
            let ms = elapsed.as_secs_f64() * 1000.0;
            let timing = format_timing(ms);
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn done(mut self) {
        self.finish_current();
    }
}

impl Default for BuildProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Format milliseconds into a human-readable timing string.
fn format_timing(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.1}s", ms / 1000.0)
    } else if ms >= 1.0 {
        format!("{:.0}ms", ms)
    } else {
        "<1ms".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timing_seconds() {
        assert_eq!(format_timing(1500.0), "1.5s");
        assert_eq!(format_timing(1000.0), "1.0s");
        assert_eq!(format_timing(2345.6), "2.3s");
    }

    #[test]
    fn test_format_timing_milliseconds() {
        assert_eq!(format_timing(500.0), "500ms");
        assert_eq!(format_timing(1.0), "1ms");
        assert_eq!(format_timing(42.7), "43ms");
    }

    #[test]
    fn test_format_timing_sub_millisecond() {
        assert_eq!(format_timing(0.5), "<1ms");
        assert_eq!(format_timing(0.0), "<1ms");
    }

    #[test]
    fn test_new_creates_instance() {
        let progress = BuildProgress::new();
        assert!(progress.spinner.is_none());
    }

    #[test]
    fn test_default_creates_instance() {
        let progress = BuildProgress::default();
        assert!(progress.spinner.is_none());
    }
}
