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
