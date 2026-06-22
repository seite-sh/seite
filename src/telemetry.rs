//! Opt-out CLI telemetry. Mirrors `update_check.rs`: best-effort, never panics,
//! never blocks the command, never affects the exit code.

use std::io::IsTerminal;
use std::path::Path;
use std::time::Duration;

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

const SITE: &str = "cli.seite.sh";
/// Default `who` ingestion endpoint. Confirm/replace with the real deployment
/// URL before release; overridable via `SEITE_TELEMETRY_ENDPOINT` (build or run).
const DEFAULT_ENDPOINT: &str = "https://who.seite.sh/api/event";

/// Coarse duration bucket — never a raw timing.
fn duration_bucket(d: Duration) -> &'static str {
    match d.as_secs() {
        0 => "<1s",
        1..=4 => "1-5s",
        5..=29 => "5-30s",
        _ => "30s+",
    }
}

fn resolve_endpoint(runtime: Option<String>, compiled: Option<&str>) -> String {
    runtime
        .filter(|s| !s.trim().is_empty())
        .or_else(|| compiled.map(|s| s.to_string()))
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
}

fn endpoint() -> String {
    resolve_endpoint(
        std::env::var("SEITE_TELEMETRY_ENDPOINT").ok(),
        option_env!("SEITE_TELEMETRY_ENDPOINT"),
    )
}

const HTTP_TIMEOUT: Duration = Duration::from_secs(2);

fn send(payload: serde_json::Value) {
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .build(),
    );
    let ua = format!("seite-cli/{}", env!("CARGO_PKG_VERSION"));
    let _ = agent
        .post(&endpoint())
        .header("User-Agent", &ua)
        .send_json(&payload);
}

/// Show the one-time opt-out notice on an interactive stderr, then mark it shown.
fn maybe_show_notice() {
    let Some(path) = config_path() else { return };
    let mut cfg = load_config(&path);
    if cfg.notice_shown || !std::io::stderr().is_terminal() {
        return;
    }
    eprintln!(
        "seite collects anonymous usage telemetry (command name, version, OS) to improve the tool.\n\
         No paths, content, or personal data are sent. Disable with `seite telemetry off` or DO_NOT_TRACK=1.\n\
         Details: https://seite.sh/telemetry"
    );
    cfg.notice_shown = true;
    let _ = write_config(&path, &cfg);
}

/// Record one command invocation. Best-effort, detached; never blocks meaningfully.
pub fn maybe_record_command(command: &str, success: bool, duration: Duration) {
    if !is_enabled() {
        return;
    }
    maybe_show_notice();
    let payload = build_command_payload(
        command,
        success,
        duration,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    // Detached: do not join. A very fast command may exit before the send
    // completes — acceptable for rough metrics.
    let _ = std::thread::Builder::new()
        .name("seite-telemetry".into())
        .spawn(move || send(payload));
}

fn build_command_payload(
    command: &str,
    success: bool,
    duration: Duration,
    version: &str,
    os: &str,
    arch: &str,
) -> serde_json::Value {
    serde_json::json!({
        "name": "command",
        "domain": SITE,
        "url": format!("https://{SITE}/cmd/{command}"),
        "props": {
            "version": version,
            "os": os,
            "arch": arch,
            "success": success,
            "duration_bucket": duration_bucket(duration),
        }
    })
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

    #[test]
    fn duration_bucket_is_coarse() {
        use std::time::Duration;
        assert_eq!(duration_bucket(Duration::from_millis(200)), "<1s");
        assert_eq!(duration_bucket(Duration::from_secs(3)), "1-5s");
        assert_eq!(duration_bucket(Duration::from_secs(20)), "5-30s");
        assert_eq!(duration_bucket(Duration::from_secs(120)), "30s+");
    }

    #[test]
    fn endpoint_prefers_runtime_then_compiled_then_default() {
        assert_eq!(resolve_endpoint(Some("https://rt".into()), Some("https://ct")), "https://rt");
        assert_eq!(resolve_endpoint(Some("  ".into()), Some("https://ct")), "https://ct");
        assert_eq!(resolve_endpoint(None, Some("https://ct")), "https://ct");
        assert_eq!(resolve_endpoint(None, None), DEFAULT_ENDPOINT);
    }

    #[test]
    fn command_payload_has_only_allowed_fields() {
        use std::time::Duration;
        let v = build_command_payload("build", true, Duration::from_secs(2), "0.12.2", "linux", "x86_64");
        assert_eq!(v["name"], "command");
        assert_eq!(v["domain"], "cli.seite.sh");
        assert_eq!(v["url"], "https://cli.seite.sh/cmd/build");
        let props = v["props"].as_object().unwrap();
        let mut keys: Vec<&String> = props.keys().collect();
        keys.sort();
        assert_eq!(keys, vec!["arch", "duration_bucket", "os", "success", "version"]);
        assert_eq!(props["version"], "0.12.2");
        assert_eq!(props["success"], true);
        assert_eq!(props["duration_bucket"], "1-5s");
    }
}
