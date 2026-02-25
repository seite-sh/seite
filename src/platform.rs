/// Platform-specific helpers for Windows compatibility.
use std::path::PathBuf;
use std::process::Command;

/// Resolve an npm-installed CLI command name for the current platform.
///
/// On Windows, npm global installs create `.cmd` shims (e.g., `claude.cmd`,
/// `wrangler.cmd`) which `Command::new` cannot find — it only searches for
/// `.exe` files. This function appends `.cmd` on Windows so the command
/// resolves correctly.
///
/// Commands that ship as native `.exe` (like `git`) should NOT use this —
/// call `Command::new("git")` directly.
pub fn npm_cmd(name: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", name]);
        cmd
    } else {
        Command::new(name)
    }
}

/// Resolve the user's home directory in a cross-platform way.
///
/// - Unix/macOS: `$HOME`
/// - Windows: `%USERPROFILE%`
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Resolve a path relative to the user's home directory.
pub fn home_path(relative: &str) -> Option<PathBuf> {
    home_dir().map(|home| home.join(relative))
}

/// Resolve the wrangler CLI config file path.
///
/// - macOS: `~/Library/Preferences/.wrangler/config/default.toml`
/// - Windows: `%APPDATA%/.wrangler/config/default.toml`
/// - Linux: `~/.config/.wrangler/config/default.toml`
pub fn wrangler_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        home_path("Library/Preferences/.wrangler/config/default.toml")
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join(".wrangler/config/default.toml"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        home_path(".config/.wrangler/config/default.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npm_cmd_creates_command() {
        let cmd = npm_cmd("claude");
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(windows) {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "claude");
        }
    }

    #[test]
    fn test_home_dir_returns_some() {
        // HOME is set in most test environments
        let home = home_dir();
        assert!(home.is_some());
        assert!(home.unwrap().is_absolute());
    }

    #[test]
    fn test_home_path_appends_relative() {
        let p = home_path(".seite/config.json");
        assert!(p.is_some());
        let path = p.unwrap();
        assert!(path.to_string_lossy().ends_with(".seite/config.json"));
    }

    #[test]
    fn test_wrangler_config_path_returns_some() {
        let p = wrangler_config_path();
        assert!(p.is_some());
        let path = p.unwrap();
        assert!(path.to_string_lossy().contains("wrangler"));
        assert!(path.to_string_lossy().ends_with("default.toml"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_wrangler_config_path_macos() {
        let p = wrangler_config_path().unwrap();
        assert!(p
            .to_string_lossy()
            .contains("Library/Preferences/.wrangler"));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn test_wrangler_config_path_linux() {
        let p = wrangler_config_path().unwrap();
        assert!(p.to_string_lossy().contains(".config/.wrangler"));
    }
}
