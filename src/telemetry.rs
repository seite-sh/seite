//! Opt-out CLI telemetry. Mirrors `update_check.rs`: best-effort, never panics,
//! never blocks the command, never affects the exit code.

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
    }
}
