//! Opt-out CLI telemetry. Mirrors `update_check.rs`: best-effort, never panics,
//! never blocks the command, never affects the exit code.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::platform;

const TELEMETRY_DIR: &str = ".seite";
const TELEMETRY_FILE: &str = "telemetry.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TelemetryConfig {
    /// `None` = user never set a preference (opt-out default applies).
    enabled: Option<bool>,
    /// Whether the one-time first-run notice has been shown.
    notice_shown: bool,
}

fn config_path() -> Option<std::path::PathBuf> {
    platform::home_dir().map(|home| home.join(TELEMETRY_DIR).join(TELEMETRY_FILE))
}

fn load_config(path: &Path) -> TelemetryConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_config(path: &Path, cfg: &TelemetryConfig) -> Option<()> {
    let parent = path.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    let json = serde_json::to_string_pretty(cfg).ok()?;
    std::fs::write(path, json).ok()?;
    Some(())
}

fn env_nonempty(key: &str) -> bool {
    std::env::var(key).map(|v| !v.trim().is_empty()).unwrap_or(false)
}

/// Full opt-out decision using real env + saved config.
pub fn decision() -> Decision {
    let cfg = config_path().map(|p| load_config(&p)).unwrap_or_default();
    resolve(
        env_nonempty("DO_NOT_TRACK"),
        std::env::var("SEITE_TELEMETRY").ok().and_then(|v| parse_flag(&v)),
        env_nonempty("CI"),
        cfg.enabled,
    )
}

pub fn is_enabled() -> bool {
    decision().is_enabled()
}

/// Persist an explicit on/off preference (`seite telemetry on|off`).
pub fn set_enabled(on: bool) -> Option<()> {
    let path = config_path()?;
    let mut cfg = load_config(&path);
    cfg.enabled = Some(on);
    write_config(&path, &cfg)
}

/// One-line human status for `seite telemetry status`.
pub fn status_line() -> String {
    let d = decision();
    let state = if d.is_enabled() { "enabled" } else { "disabled" };
    let reason = match d {
        Decision::DisabledByDoNotTrack => "DO_NOT_TRACK is set",
        Decision::EnabledByEnv | Decision::DisabledByEnv => "SEITE_TELEMETRY env override",
        Decision::DisabledByCi => "CI environment detected",
        Decision::EnabledByConfig | Decision::DisabledByConfig => "saved preference",
        Decision::EnabledByDefault => "default (opt-out)",
    };
    format!("Telemetry is {state} ({reason}).")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    DisabledByDoNotTrack,
    EnabledByEnv,
    DisabledByEnv,
    DisabledByCi,
    EnabledByConfig,
    DisabledByConfig,
    EnabledByDefault,
}

impl Decision {
    pub fn is_enabled(&self) -> bool {
        matches!(
            self,
            Decision::EnabledByEnv | Decision::EnabledByConfig | Decision::EnabledByDefault
        )
    }
}

/// Parse a telemetry on/off flag value. Returns `None` for unrecognized values.
fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "yes" | "enable" | "enabled" => Some(true),
        "0" | "off" | "false" | "no" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

/// Pure precedence resolver (unit-tested). See Global Constraints for order.
fn resolve(dnt: bool, env_flag: Option<bool>, ci: bool, cfg: Option<bool>) -> Decision {
    if dnt {
        return Decision::DisabledByDoNotTrack;
    }
    if let Some(v) = env_flag {
        return if v { Decision::EnabledByEnv } else { Decision::DisabledByEnv };
    }
    if ci {
        return Decision::DisabledByCi;
    }
    if let Some(v) = cfg {
        return if v { Decision::EnabledByConfig } else { Decision::DisabledByConfig };
    }
    Decision::EnabledByDefault
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flag_recognizes_values() {
        assert_eq!(parse_flag("0"), Some(false));
        assert_eq!(parse_flag("off"), Some(false));
        assert_eq!(parse_flag("NO"), Some(false));
        assert_eq!(parse_flag("1"), Some(true));
        assert_eq!(parse_flag("on"), Some(true));
        assert_eq!(parse_flag("garbage"), None);
        assert_eq!(parse_flag(" on "), Some(true));
        assert_eq!(parse_flag("enable"), Some(true));
        assert_eq!(parse_flag("disable"), Some(false));
    }

    #[test]
    fn resolve_precedence() {
        // DO_NOT_TRACK wins over everything.
        assert_eq!(resolve(true, Some(true), false, Some(true)), Decision::DisabledByDoNotTrack);
        // env flag beats CI and config.
        assert_eq!(resolve(false, Some(false), false, Some(true)), Decision::DisabledByEnv);
        assert_eq!(resolve(false, Some(true), true, Some(false)), Decision::EnabledByEnv);
        // CI beats config.
        assert_eq!(resolve(false, None, true, Some(true)), Decision::DisabledByCi);
        // config beats default.
        assert_eq!(resolve(false, None, false, Some(false)), Decision::DisabledByConfig);
        assert_eq!(resolve(false, None, false, Some(true)), Decision::EnabledByConfig);
        // default is enabled (opt-out).
        assert_eq!(resolve(false, None, false, None), Decision::EnabledByDefault);
    }

    #[test]
    fn is_enabled_maps_decisions() {
        assert!(Decision::EnabledByDefault.is_enabled());
        assert!(Decision::EnabledByEnv.is_enabled());
        assert!(Decision::EnabledByConfig.is_enabled());
        assert!(!Decision::DisabledByCi.is_enabled());
        assert!(!Decision::DisabledByDoNotTrack.is_enabled());
        assert!(!Decision::DisabledByEnv.is_enabled());
        assert!(!Decision::DisabledByConfig.is_enabled());
    }

    #[test]
    fn config_roundtrips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("seite-tel-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("telemetry.json");

        // Missing file -> default config (enabled None, notice not shown).
        let cfg = load_config(&path);
        assert_eq!(cfg.enabled, None);
        assert!(!cfg.notice_shown);

        // Write then read back.
        let written = TelemetryConfig { enabled: Some(false), notice_shown: true };
        write_config(&path, &written).unwrap();
        let read = load_config(&path);
        assert_eq!(read.enabled, Some(false));
        assert!(read.notice_shown);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn status_line_reports_state() {
        // Pure-ish: just assert it renders a sentence containing "Telemetry is".
        let s = status_line();
        assert!(s.starts_with("Telemetry is "));
    }
}
