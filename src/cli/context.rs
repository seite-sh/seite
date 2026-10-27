//! `seite context` — once-per-session orientation for coding agents: what
//! the site is, where things live, what is running, and DIRECTIVES (things
//! the agent should do or know), each a `NAME: message` line.
//!
//! Works outside a seite project too (it says so and stops), so an agent can
//! run it unconditionally at the start of a session.

use std::path::Path;

use clap::Args;
use console::style;
use serde::Serialize;
use serde_json::{json, Value};

use super::site_tools::{array_of, call_tool, load_state, no_site_message, str_of};
use crate::human_println;
use crate::mcp::ServerState;
use crate::output::json as json_out;
use crate::{meta, themes};

#[derive(Args)]
pub struct ContextArgs {}

/// Something the agent should act on or keep in mind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Directive {
    pub name: &'static str,
    pub message: String,
}

/// The facts the directives are computed from (kept apart from I/O so the
/// rules are unit-testable).
#[derive(Debug, Clone, Default)]
pub struct DirectiveInputs {
    /// Version recorded in `.seite/config.json`; `None` when the file is missing.
    pub project_version: Option<(u64, u64, u64)>,
    pub binary_version: (u64, u64, u64),
    /// Any turn-end `seite check` hook recorded in `.seite/config.json`.
    pub has_stop_hook: bool,
    /// Active theme as reported by [`themes::active_theme`].
    pub active_theme: String,
    /// An `impeccable` skill folder exists for some agent.
    pub impeccable_installed: bool,
    /// `seite.toml` exists but failed to load.
    pub config_error: Option<String>,
    /// `[build] schedule = true` without a `[site] timezone`.
    pub schedule_without_timezone: bool,
    /// The running dev server's last build failed, with this many errors.
    pub dev_build_errors: Option<u64>,
    /// Browser errors the running dev server has recorded.
    pub browser_errors: u64,
}

/// The directives for a site, in a stable order.
pub fn compute_directives(inputs: &DirectiveInputs) -> Vec<Directive> {
    let mut out = Vec::new();
    if let Some(err) = &inputs.config_error {
        out.push(Directive {
            name: "CONFIG_INVALID",
            message: format!(
                "seite.toml does not load ({}); fix it first (`seite check` lists every problem)",
                err.lines().next().unwrap_or_default()
            ),
        });
    }
    let binary = meta::format_version(inputs.binary_version);
    match inputs.project_version {
        None => out.push(Directive {
            name: "UPGRADE_AVAILABLE",
            message: format!(
                "no .seite/config.json (project predates seite's agent setup); run `seite upgrade` to add agent files for seite {binary}"
            ),
        }),
        Some(v) if v < inputs.binary_version => out.push(Directive {
            name: "UPGRADE_AVAILABLE",
            message: format!(
                "project was set up with seite {}, this binary is {binary}; run `seite upgrade` to refresh agent files and config",
                meta::format_version(v)
            ),
        }),
        Some(_) => {}
    }
    if !inputs.has_stop_hook {
        out.push(Directive {
            name: "NO_STOP_HOOK",
            message: "no turn-end check hook is installed; run `seite check` before finishing and fix every error it reports".into(),
        });
    }
    if inputs.schedule_without_timezone {
        out.push(Directive {
            name: "TIMEZONE_UNSET",
            message: "[build] schedule is on but [site] timezone is unset, so what is live depends on each machine's clock (CI often runs in UTC); ask the user for their zone and add `timezone = \"<IANA name>\"` (e.g. \"Europe/Berlin\") under [site]".into(),
        });
    }
    if let Some(errors) = inputs.dev_build_errors {
        out.push(Directive {
            name: "DEV_BUILD_FAILED",
            message: format!(
                "the running dev server's last build failed ({errors} error{}); run `seite preview` to read them, fix, and save",
                if errors == 1 { "" } else { "s" }
            ),
        });
    }
    if inputs.browser_errors > 0 {
        out.push(Directive {
            name: "BROWSER_ERRORS",
            message: format!(
                "preview pages reported {} JavaScript error{}; run `seite preview` to read them. {}",
                inputs.browser_errors,
                if inputs.browser_errors == 1 { "" } else { "s" },
                crate::server::dev::BROWSER_ERRORS_NOTE
            ),
        });
    }
    let plain_theme = inputs.active_theme == "custom" || inputs.active_theme == "default";
    if plain_theme && !inputs.impeccable_installed {
        out.push(Directive {
            name: "ADDON_SUGGESTION",
            message: "optional: for design work on templates/base.html, `seite skill install impeccable` adds design skills (suggest it to the user; don't install unasked)".into(),
        });
    }
    out
}

/// `.seite/dev.json`, written by a running `seite serve`.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DevServer {
    pub pid: u32,
    pub port: u16,
    pub url: String,
}

/// The dev server recorded in `.seite/dev.json`, if it is really running.
///
/// The file lives in the project, so a cloned repository could plant one;
/// nothing in it is trusted beyond the pid and port (and it is gitignored).
/// It is ignored when `.seite` or the file is a symlink; the server must be
/// listening on the loopback port (and, on unix, its pid alive); the URL is
/// rebuilt locally from the port, never read from the file. (No `git`
/// probe: running git in an untrusted repo can execute its config.)
pub(crate) fn running_dev_server(root: &Path) -> Option<DevServer> {
    let path = root.join(".seite").join("dev.json");
    crate::safe_fs::guard(root, &path).ok()?;
    if !std::fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(&path).ok()?;
    let recorded: DevServer = serde_json::from_str(&raw).ok()?;
    if recorded.port == 0 {
        return None;
    }
    let alive = process_alive(recorded.pid) && port_open(recorded.port);
    alive.then(|| DevServer {
        pid: recorded.pid,
        port: recorded.port,
        url: dev_server_url(recorded.port),
    })
}

/// The URL of a dev server on `port`, built locally (loopback).
pub(crate) fn dev_server_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// Whether something accepts TCP connections on the loopback `port`.
fn port_open(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok()
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    match rustix::process::test_kill_process(pid) {
        Ok(()) => true,
        // Exists, but owned by another user.
        Err(e) => e == rustix::io::Errno::PERM,
    }
}

// Windows (and other non-unix) has no cheap pid probe here, so liveness is
// port-probe based only ([`port_open`]).
#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    true
}

fn impeccable_installed(root: &Path) -> bool {
    [".claude/skills/impeccable", ".agents/skills/impeccable"]
        .iter()
        .any(|dir| root.join(dir).is_dir())
}

pub fn run(_args: &ContextArgs, site_filter: Option<&str>) -> anyhow::Result<()> {
    let state = load_state(site_filter)?;
    let version = env!("CARGO_PKG_VERSION");
    // Version-matched local docs, written lazily (gitignored, not committed).
    crate::cli::harness::ensure_local_docs(&state.cwd);

    if state.config.is_none() && state.config_error.is_none() {
        let message = no_site_message(&state);
        human_println!("seite {version}");
        human_println!("{message}");
        json_out::set_data(json!({
            "version": version,
            "in_project": false,
            "root": state.cwd,
            "message": message,
            "directives": [],
        }));
        return Ok(());
    }

    let data = gather(&state, version);
    print_context(&data);
    json_out::set_data(data);
    Ok(())
}

/// Everything `seite context` reports, as its `--json` document.
fn gather(state: &ServerState, version: &str) -> Value {
    let root = &state.cwd;
    let project_meta = meta::load(root);

    let site = state.config.as_ref().map(|config| {
        json!({
            "title": config.site.title,
            "base_url": config.site.base_url,
            "language": config.site.language,
            "languages": config.all_languages(),
        })
    });

    // Item and draft counts come from the same code as `seite stats`.
    let stats = call_tool(state, "seite_content_stats", json!({})).ok();
    let collections: Vec<Value> = state
        .config
        .as_ref()
        .map(|config| {
            config
                .collections
                .iter()
                .map(|c| {
                    let counts = stats.as_ref().and_then(|s| {
                        array_of(s, "collections")
                            .iter()
                            .find(|sc| str_of(sc, "name") == c.name)
                    });
                    let n = |k: &str| counts.and_then(|v| v.get(k)).and_then(Value::as_u64);
                    json!({
                        "name": c.name,
                        "dir": format!("content/{}", c.directory),
                        "url_prefix": if c.url_prefix.is_empty() { "/".to_string() } else { c.url_prefix.clone() },
                        "items": n("total"),
                        "drafts": n("drafts"),
                        "schema": (!c.fields.is_empty() || c.strict_fields)
                            .then(|| crate::content::schema::describe_collection(config, c)),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let templates_dir = state
        .paths
        .as_ref()
        .map(|p| p.templates.clone())
        .unwrap_or_else(|| root.join("templates"));
    let active_theme = themes::active_theme(root, &templates_dir);
    let base_exists = templates_dir.join("base.html").is_file();

    let local_docs = root.join(".seite").join("docs");
    let skill_packs: Vec<String> = super::skill::gather_skill_summary(root)
        .packs
        .into_iter()
        .map(|p| p.name)
        .collect();
    let impeccable = impeccable_installed(root);
    let hooks = project_meta
        .as_ref()
        .and_then(|m| m.hooks_installed.clone())
        .unwrap_or_default();

    // What the running server knows (short timeout: never slow `context` down).
    let mut dev_server = running_dev_server(root);
    let dev_status = dev_server.as_ref().and_then(|s| {
        crate::server::remote::get_json(
            s.port,
            "/__seite/status",
            std::time::Duration::from_millis(300),
        )
        .ok()
    });
    // A server that answers for another site (e.g. a recorded
    // `seite serve --site other`) is not this site's dev server.
    if let (Some(server), Some(status)) = (&dev_server, &dev_status) {
        if crate::server::remote::site_url_in(status, &server.url, root).is_err() {
            dev_server = None;
        }
    }
    let dev_status = dev_status.filter(|_| dev_server.is_some());
    let last_build = dev_status
        .as_ref()
        .and_then(|s| s.get("last_build"))
        .filter(|v| !v.is_null());
    let dev_build_errors = last_build
        .filter(|b| b.get("ok").and_then(Value::as_bool) == Some(false))
        .map(|b| b.get("errors").and_then(Value::as_u64).unwrap_or(1));
    let browser_errors = dev_status
        .as_ref()
        .map_or(0, |s| array_of(s, "browser_errors").len() as u64);
    let dev_server_json = dev_server.as_ref().map(|server| {
        let mut v = json!(server);
        v["status"] = match &dev_status {
            Some(_) => json!({
                "last_build_ok": last_build.and_then(|b| b.get("ok")).cloned(),
                "build_errors": last_build.and_then(|b| b.get("errors")).cloned(),
                "build_warnings": last_build.and_then(|b| b.get("warnings")).cloned(),
                "browser_errors": browser_errors,
            }),
            None => Value::Null,
        };
        v
    });

    let directives = compute_directives(&DirectiveInputs {
        project_version: project_meta.as_ref().map(|_| meta::project_version(root)),
        binary_version: meta::binary_version(),
        has_stop_hook: !hooks.is_empty(),
        active_theme: active_theme.clone(),
        impeccable_installed: impeccable,
        config_error: state.config_error.clone(),
        schedule_without_timezone: state
            .config
            .as_ref()
            .is_some_and(|c| c.build.schedule && c.site.timezone.is_none()),
        dev_build_errors,
        browser_errors,
    });

    json!({
        "version": version,
        "in_project": true,
        "root": root,
        "project_version": project_meta.as_ref().map(|m| m.version.clone()),
        "config_error": state.config_error,
        "site": site,
        "collections": collections,
        "theme": {
            "active": active_theme,
            "base_html": base_exists.then_some("templates/base.html"),
            "customized": active_theme == "custom",
            "tailwind": state.config.as_ref().and_then(|c| c.build.tailwind.as_ref()).map(|tw| json!({
                "input": tw.input,
                "stylesheet": format!("/static/{}", tw.output),
                "version": crate::build::tailwind::PINNED_VERSION,
                "binary_env": std::env::var_os(crate::build::tailwind::BIN_ENV).map(|v| v.to_string_lossy().to_string()),
            })),
        },
        "docs": local_docs.is_dir().then_some(".seite/docs/"),
        "skill_packs": skill_packs,
        "impeccable_installed": impeccable,
        "agents": project_meta.as_ref().and_then(|m| m.agents.clone()),
        "hooks_installed": hooks,
        "dev_server": dev_server_json,
        "today": stats.as_ref().and_then(|s| s.get("today")).cloned(),
        "next_scheduled": stats.as_ref().and_then(|s| s.get("next_scheduled")).cloned(),
        "directives": directives,
    })
}

fn print_context(data: &Value) {
    let label = |s: &str| style(format!("{s:<13}")).bold().to_string();
    human_println!("seite {}", str_of(data, "version"));
    human_println!("{}{}", label("Root:"), str_of(data, "root"));
    if let Some(site) = data.get("site").filter(|s| !s.is_null()) {
        let langs: Vec<&str> = array_of(site, "languages")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        human_println!(
            "{}{} — {} (languages: {})",
            label("Site:"),
            str_of(site, "title"),
            str_of(site, "base_url"),
            langs.join(", ")
        );
    }
    let collections = array_of(data, "collections");
    if !collections.is_empty() {
        human_println!("{}", label("Collections:"));
        for c in collections {
            let count = |k: &str| {
                c.get(k)
                    .and_then(Value::as_u64)
                    .map_or("?".to_string(), |n| n.to_string())
            };
            human_println!(
                "  {:<12} {:<20} {:<12} {} items ({} drafts)",
                str_of(c, "name"),
                str_of(c, "dir"),
                str_of(c, "url_prefix"),
                count("items"),
                count("drafts")
            );
        }
    }
    for c in collections {
        let Some(schema) = c.get("schema").filter(|s| !s.is_null()) else {
            continue;
        };
        let fields: Vec<String> = array_of(schema, "fields")
            .iter()
            .map(|f| {
                let required = if f.get("required").and_then(Value::as_bool) == Some(true) {
                    ", required"
                } else {
                    ""
                };
                format!(
                    "{} ({}{required})",
                    str_of(f, "location").replace("top-level", str_of(f, "name")),
                    str_of(f, "type")
                )
            })
            .collect();
        let strict = if schema.get("strict_fields").and_then(Value::as_bool) == Some(true) {
            " [strict]"
        } else {
            ""
        };
        human_println!(
            "{}{}: {}{strict}",
            label("Schema:"),
            str_of(c, "name"),
            fields.join(", ")
        );
    }
    if let Some(next) = data.get("next_scheduled").filter(|v| !v.is_null()) {
        human_println!(
            "{}next scheduled: {} ({})",
            label("Scheduled:"),
            str_of(next, "date"),
            str_of(next, "title")
        );
    }
    let theme = data.get("theme").cloned().unwrap_or(json!({}));
    let base = match theme.get("base_html").and_then(Value::as_str) {
        None => "no templates/base.html (built-in default)",
        Some(_) if theme.get("customized").and_then(Value::as_bool) == Some(true) => {
            "templates/base.html is customized"
        }
        Some(_) => "templates/base.html matches it",
    };
    human_println!("{}{} ({base})", label("Theme:"), str_of(&theme, "active"));
    if let Some(tw) = theme.get("tailwind").filter(|v| !v.is_null()) {
        human_println!(
            "{}{} → {} (v{}; link the stylesheet URL in base.html)",
            label("Tailwind:"),
            str_of(tw, "input"),
            str_of(tw, "stylesheet"),
            str_of(tw, "version")
        );
    }
    match data.get("docs").and_then(Value::as_str) {
        Some(dir) => human_println!(
            "{}{dir} (local, matches this seite version; `seite docs <query>` searches)",
            label("Docs:")
        ),
        None => human_println!(
            "{}`seite docs` (topics) / `seite docs <query>` / https://seite.sh/docs",
            label("Docs:")
        ),
    }
    let list = |key: &str| {
        let items: Vec<&str> = array_of(data, key)
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if items.is_empty() {
            "(none)".to_string()
        } else {
            items.join(", ")
        }
    };
    human_println!("{}{}", label("Skill packs:"), list("skill_packs"));
    human_println!(
        "{}{} (hooks: {})",
        label("Agents:"),
        list("agents"),
        list("hooks_installed")
    );
    match data.get("dev_server").filter(|v| !v.is_null()) {
        Some(server) => human_println!(
            "{}{} (pid {}){}",
            label("Dev server:"),
            str_of(server, "url"),
            server.get("pid").and_then(Value::as_u64).unwrap_or(0),
            dev_status_suffix(server.get("status"))
        ),
        None => human_println!("{}not running (`seite serve`)", label("Dev server:")),
    }
    human_println!(
        "{}`seite check` after changes; `seite audit` for SEO, AI-visibility and accessibility fixes",
        label("Next:")
    );
    let directives = array_of(data, "directives");
    if !directives.is_empty() {
        human_println!();
        human_println!("DIRECTIVES");
        for d in directives {
            human_println!("{}: {}", str_of(d, "name"), str_of(d, "message"));
        }
    }
}

/// `, last build ok, 0 browser errors` (empty when the status is unknown).
fn dev_status_suffix(status: Option<&Value>) -> String {
    let Some(status) = status.filter(|s| !s.is_null()) else {
        return String::new();
    };
    let build = match status.get("last_build_ok").and_then(Value::as_bool) {
        Some(true) => "last build ok".to_string(),
        Some(false) => format!(
            "last build FAILED ({} errors)",
            status
                .get("build_errors")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        None => "no build yet".to_string(),
    };
    let browser = status
        .get("browser_errors")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!(", {build}, {browser} browser errors (`seite preview`)")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(d: &[Directive]) -> Vec<&'static str> {
        d.iter().map(|d| d.name).collect()
    }

    fn up_to_date() -> DirectiveInputs {
        DirectiveInputs {
            project_version: Some((0, 21, 0)),
            binary_version: (0, 21, 0),
            has_stop_hook: true,
            active_theme: "dark".into(),
            impeccable_installed: false,
            config_error: None,
            schedule_without_timezone: false,
            dev_build_errors: None,
            browser_errors: 0,
        }
    }

    #[test]
    fn test_dev_server_directives() {
        let mut inputs = up_to_date();
        inputs.dev_build_errors = Some(2);
        inputs.browser_errors = 1;
        let d = compute_directives(&inputs);
        assert_eq!(names(&d), ["DEV_BUILD_FAILED", "BROWSER_ERRORS"]);
        assert!(d[0].message.contains("2 errors") && d[0].message.contains("seite preview"));
        assert!(d[1].message.contains("1 JavaScript error;"));
    }

    #[test]
    fn test_dev_status_suffix() {
        assert_eq!(dev_status_suffix(None), "");
        let s = json!({"last_build_ok": false, "build_errors": 3, "browser_errors": 1});
        assert_eq!(
            dev_status_suffix(Some(&s)),
            ", last build FAILED (3 errors), 1 browser errors (`seite preview`)"
        );
    }

    #[test]
    fn test_timezone_unset_directive() {
        let mut inputs = up_to_date();
        inputs.schedule_without_timezone = true;
        let d = compute_directives(&inputs);
        assert_eq!(names(&d), ["TIMEZONE_UNSET"]);
        assert!(d[0].message.contains("timezone = "));
    }

    #[test]
    fn test_no_directives_when_everything_is_set_up() {
        assert!(compute_directives(&up_to_date()).is_empty());
    }

    #[test]
    fn test_upgrade_available_when_project_is_older_or_unrecorded() {
        let mut inputs = up_to_date();
        inputs.project_version = Some((0, 20, 1));
        let d = compute_directives(&inputs);
        assert_eq!(names(&d), ["UPGRADE_AVAILABLE"]);
        assert!(d[0].message.contains("0.20.1") && d[0].message.contains("seite upgrade"));

        inputs.project_version = None;
        assert_eq!(names(&compute_directives(&inputs)), ["UPGRADE_AVAILABLE"]);

        // A newer project (older binary) is not an upgrade.
        inputs.project_version = Some((0, 22, 0));
        assert!(compute_directives(&inputs).is_empty());
    }

    #[test]
    fn test_no_stop_hook() {
        let mut inputs = up_to_date();
        inputs.has_stop_hook = false;
        let d = compute_directives(&inputs);
        assert_eq!(names(&d), ["NO_STOP_HOOK"]);
        assert!(d[0].message.contains("seite check"));
    }

    #[test]
    fn test_addon_suggestion_only_for_default_or_custom_theme_without_impeccable() {
        let mut inputs = up_to_date();
        for theme in ["default", "custom"] {
            inputs.active_theme = theme.into();
            inputs.impeccable_installed = false;
            assert_eq!(names(&compute_directives(&inputs)), ["ADDON_SUGGESTION"]);
            inputs.impeccable_installed = true;
            assert!(compute_directives(&inputs).is_empty());
        }
        inputs.active_theme = "brutalist".into();
        inputs.impeccable_installed = false;
        assert!(compute_directives(&inputs).is_empty());
    }

    #[test]
    fn test_directive_order_and_config_error() {
        let inputs = DirectiveInputs {
            project_version: None,
            binary_version: (0, 21, 0),
            has_stop_hook: false,
            active_theme: "custom".into(),
            impeccable_installed: false,
            config_error: Some("Failed to load seite.toml: bad\nmore".into()),
            schedule_without_timezone: false,
            dev_build_errors: None,
            browser_errors: 0,
        };
        let d = compute_directives(&inputs);
        assert_eq!(
            names(&d),
            [
                "CONFIG_INVALID",
                "UPGRADE_AVAILABLE",
                "NO_STOP_HOOK",
                "ADDON_SUGGESTION"
            ]
        );
        assert!(!d[0].message.contains("more"));
    }

    #[test]
    fn test_dev_server_requires_live_pid_and_open_port() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(running_dev_server(tmp.path()).is_none());
        std::fs::create_dir_all(tmp.path().join(".seite")).unwrap();
        let own = std::process::id();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // A foreign URL in the file is never reported: it's rebuilt locally.
        std::fs::write(
            tmp.path().join(".seite/dev.json"),
            format!(r#"{{"pid":{own},"port":{port},"url":"https://evil.example"}}"#),
        )
        .unwrap();
        assert_eq!(
            running_dev_server(tmp.path()).map(|s| s.url),
            Some(format!("http://127.0.0.1:{port}"))
        );
        drop(listener);
        assert!(running_dev_server(tmp.path()).is_none(), "port closed");
        std::fs::write(tmp.path().join(".seite/dev.json"), "not json").unwrap();
        assert!(running_dev_server(tmp.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_dev_server_ignores_symlinked_state() {
        let tmp = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::fs::write(
            outside.path().join("dev.json"),
            format!(
                r#"{{"pid":{},"port":{port},"url":"x"}}"#,
                std::process::id()
            ),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join(".seite")).unwrap();
        assert!(running_dev_server(tmp.path()).is_none());
    }
}
