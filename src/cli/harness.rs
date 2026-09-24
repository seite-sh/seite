//! Coding-agent harness files for generated sites.
//!
//! A seite site can be set up for several coding agents at once: Claude Code,
//! OpenAI Codex CLI, OpenCode, and Cursor (editor + `cursor-agent`). Each one
//! reads project context from different places, so this module declares every
//! rules file, skill, and the MCP server entry **once** and renders them into
//! each agent's format:
//!
//! | Agent    | MCP config           | Path-scoped rules        | Skills            |
//! |----------|----------------------|--------------------------|-------------------|
//! | claude   | `.mcp.json`          | `.claude/rules/*.md`     | `.claude/skills/` |
//! | cursor   | `.cursor/mcp.json`   | `.cursor/rules/*.mdc`    | `.agents/skills/` |
//! | codex    | `.codex/config.toml` | (index in AGENTS.md)     | `.agents/skills/` |
//! | opencode | `opencode.json`      | (index in AGENTS.md)     | `.agents/skills/` |
//!
//! `seite init` writes [`plan`]; `seite upgrade` merges the same content into
//! existing projects (see `cli::upgrade`).

use std::path::Path;

use crate::cli::prompt;
use crate::config::SiteConfig;
use crate::output::human;

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

/// A coding agent that `seite init` / `seite upgrade` can set a site up for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Agent {
    Claude,
    Codex,
    Opencode,
    Cursor,
}

impl Agent {
    /// Every supported agent, in canonical order.
    pub const ALL: [Agent; 4] = [Agent::Claude, Agent::Codex, Agent::Opencode, Agent::Cursor];

    /// The identifier used by `--agents` and stored in `.seite/config.json`.
    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Opencode => "opencode",
            Agent::Cursor => "cursor",
        }
    }

    /// Human-readable product name.
    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex CLI",
            Agent::Opencode => "OpenCode",
            Agent::Cursor => "Cursor",
        }
    }

    pub fn from_id(id: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// Comma-separated list of every agent id (for help and error messages).
pub fn all_ids() -> String {
    Agent::ALL.map(Agent::id).join(", ")
}

/// Parse a comma-separated `--agents` value. Accepts `all`; rejects unknown
/// names with a did-you-mean hint. Returns agents deduplicated in canonical
/// order.
pub fn parse_agent_list(list: &str) -> anyhow::Result<Vec<Agent>> {
    let ids: Vec<&str> = Agent::ALL.iter().map(|a| a.id()).collect();
    let mut agents = Vec::new();
    for raw in list.split(',') {
        let name = raw.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if name == "all" {
            agents.extend(Agent::ALL);
            continue;
        }
        match Agent::from_id(&name) {
            Some(agent) => agents.push(agent),
            None => anyhow::bail!(
                "unknown agent '{name}'. Valid agents: {}, all{}",
                all_ids(),
                human::suggest_match(&name, &ids)
            ),
        }
    }
    agents.sort();
    agents.dedup();
    if agents.is_empty() {
        anyhow::bail!("--agents needs at least one agent ({})", all_ids());
    }
    Ok(agents)
}

/// Resolve the agent selection for `seite init`: the `--agents` flag, else an
/// interactive multi-select with everything preselected, else (not
/// interactive) every agent.
pub fn resolve_init_agents(flag: Option<&str>) -> anyhow::Result<Vec<Agent>> {
    if let Some(list) = flag {
        return parse_agent_list(list);
    }
    let labels: Vec<&str> = Agent::ALL.iter().map(|a| a.label()).collect();
    let selected = prompt::multi_select(
        "Coding agents to set up (space toggles)",
        &labels,
        &[true; Agent::ALL.len()],
    )?;
    if selected.is_empty() {
        anyhow::bail!(
            "select at least one coding agent (or pass --agents {})",
            all_ids()
        );
    }
    Ok(selected.into_iter().map(|i| Agent::ALL[i]).collect())
}

/// Agent ids for persisting in `.seite/config.json`.
pub fn to_ids(agents: &[Agent]) -> Vec<String> {
    agents.iter().map(|a| a.id().to_string()).collect()
}

/// Agents from a stored id list. Unknown ids (e.g. written by a newer seite)
/// are ignored.
pub fn from_ids(ids: &[String]) -> Vec<Agent> {
    let mut agents: Vec<Agent> = ids.iter().filter_map(|id| Agent::from_id(id)).collect();
    agents.sort();
    agents.dedup();
    agents
}

fn has(agents: &[Agent], agent: Agent) -> bool {
    agents.contains(&agent)
}

/// Whether the shared `.agents/skills/` directory is needed (Codex reads only
/// that; Cursor reads it natively; OpenCode reads it alongside `.claude/skills`).
pub fn uses_shared_skills(agents: &[Agent]) -> bool {
    has(agents, Agent::Codex) || has(agents, Agent::Opencode) || has(agents, Agent::Cursor)
}

/// Where the rules files that AGENTS.md points at live, and their extension.
///
/// Claude's `.claude/rules/` when Claude is selected, else Cursor's
/// `.cursor/rules/`, else a neutral `.agents/rules/` (Codex/OpenCode have no
/// path-scoped rules, so they read these on demand via the AGENTS.md index).
pub fn rules_location(agents: &[Agent]) -> (&'static str, &'static str) {
    if has(agents, Agent::Claude) {
        (".claude/rules", "md")
    } else if has(agents, Agent::Cursor) {
        (".cursor/rules", "mdc")
    } else {
        (".agents/rules", "md")
    }
}

/// Whether the neutral `.agents/rules/` copy is the index target.
pub fn uses_shared_rules(agents: &[Agent]) -> bool {
    rules_location(agents).0 == ".agents/rules"
}

// ---------------------------------------------------------------------------
// Site features (decide conditional rules and skills)
// ---------------------------------------------------------------------------

/// Site features that make some rules/skills relevant.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SiteFeatures {
    pub pages: bool,
    pub contact: bool,
    pub trust: bool,
}

impl SiteFeatures {
    pub fn from_config(config: &SiteConfig) -> Self {
        Self {
            pages: config.collections.iter().any(|c| c.name == "pages"),
            contact: config.contact.is_some(),
            trust: config.collections.iter().any(|c| c.name == "trust"),
        }
    }

    /// Detect features of an existing project from its `seite.toml`, falling
    /// back to a textual scan when the config doesn't parse.
    pub fn detect(root: &Path) -> Self {
        let path = root.join("seite.toml");
        if let Ok(config) = SiteConfig::load(&path) {
            return Self::from_config(&config);
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        Self {
            pages: text.contains("name = \"pages\""),
            contact: text.contains("[contact]"),
            trust: text.contains("name = \"trust\""),
        }
    }
}

/// When a rule or skill applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    Always,
    Pages,
    Contact,
    Trust,
}

impl Needs {
    fn applies(self, features: SiteFeatures) -> bool {
        match self {
            Needs::Always => true,
            Needs::Pages => features.pages,
            Needs::Contact => features.contact,
            Needs::Trust => features.trust,
        }
    }
}

// ---------------------------------------------------------------------------
// Rules (path-scoped context)
// ---------------------------------------------------------------------------

/// A path-scoped context guide, rendered as `.claude/rules/<name>.md`,
/// `.cursor/rules/<name>.mdc`, and indexed in AGENTS.md.
#[derive(Debug)]
pub struct Rule {
    /// File stem (`templates` → `templates.md` / `templates.mdc`).
    pub name: &'static str,
    /// Globs the rule applies to (Claude `paths:`, Cursor `globs:`).
    pub paths: &'static [&'static str],
    /// One-line summary (Cursor `description:`).
    pub description: &'static str,
    pub content: &'static str,
    pub needs: Needs,
}

/// Every rules file, in the order they're written.
pub const RULES: &[Rule] = &[
    Rule {
        name: "seo-requirements",
        paths: &["templates/**"],
        description:
            "SEO and GEO requirements every page template must meet (meta tags, JSON-LD, feeds)",
        content: include_str!("../scaffold/seo-requirements.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "templates",
        paths: &["templates/**"],
        description: "Tera template variables, blocks, and theme conventions",
        content: include_str!("../scaffold/templates.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "i18n",
        paths: &["content/**", "templates/**", "data/i18n/**"],
        description: "Multi-language content, translations, and localized templates",
        content: include_str!("../scaffold/i18n.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "data-files",
        paths: &["data/**"],
        description: "Data files in data/ and how templates read them",
        content: include_str!("../scaffold/data-files.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "shortcodes",
        paths: &["content/**", "templates/shortcodes/**"],
        description: "Built-in and custom shortcodes for markdown content",
        content: include_str!("../scaffold/shortcodes.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "config-reference",
        paths: &["seite.toml"],
        description: "Optional seite.toml configuration sections",
        content: include_str!("../scaffold/config-reference.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "features",
        paths: &["content/**", "templates/**"],
        description:
            "Built-in site features (highlighting, homepage content, math, diagrams, search)",
        content: include_str!("../scaffold/features.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "design-prompts",
        paths: &["templates/**"],
        description: "Starting directions for redesigning or creating a theme",
        content: include_str!("../scaffold/design-prompts.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "private-collections",
        paths: &["seite.toml", "content/**"],
        description: "Private collections and password-protected access",
        content: include_str!("../scaffold/private-collections.md"),
        needs: Needs::Always,
    },
    Rule {
        name: "contact-form",
        paths: &["content/**", "templates/**", "seite.toml"],
        description: "Contact form shortcode and provider configuration",
        content: include_str!("../scaffold/contact-form.md"),
        needs: Needs::Contact,
    },
    Rule {
        name: "trust-center",
        paths: &["content/trust/**", "data/trust/**"],
        description: "Trust center data files, content, and management workflows",
        content: include_str!("../scaffold/rules-trust-center.md"),
        needs: Needs::Trust,
    },
];

/// The rules that apply to a site with `features`.
pub fn rules(features: SiteFeatures) -> impl Iterator<Item = &'static Rule> {
    RULES.iter().filter(move |r| r.needs.applies(features))
}

/// Look up a rule by name.
pub fn rule(name: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.name == name)
}

/// Wrap rule content with Claude Code's `.claude/rules/` frontmatter.
pub(crate) fn rules_file(paths: &[&str], content: &str) -> String {
    let mut result = String::with_capacity(content.len() + 128);
    result.push_str("---\npaths:\n");
    for p in paths {
        result.push_str(&format!("  - \"{p}\"\n"));
    }
    result.push_str("---\n");
    result.push_str(content);
    result
}

/// `.claude/rules/<name>.md` (also used for the neutral `.agents/rules/` copy).
pub fn claude_rule(rule: &Rule) -> String {
    rules_file(rule.paths, rule.content)
}

/// `.cursor/rules/<name>.mdc`: Cursor's MDC frontmatter. `globs` is one
/// comma-separated string (Cursor's own format, quoted because globs contain
/// `*`), and `alwaysApply: false` makes it auto-attach only for matching files.
pub fn cursor_rule(rule: &Rule) -> String {
    let globs = rule.paths.join(", ");
    let mut result = String::with_capacity(rule.content.len() + 256);
    result.push_str("---\n");
    result.push_str(&format!("description: {}\n", yaml_quote(rule.description)));
    result.push_str(&format!("globs: {}\n", yaml_quote(&globs)));
    result.push_str("alwaysApply: false\n");
    result.push_str("---\n");
    result.push_str(rule.content);
    result
}

fn yaml_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The compact rules index for AGENTS.md: each glob (or set of globs sharing
/// the same guides) with the rule files covering it.
pub fn rules_index(features: SiteFeatures, agents: &[Agent]) -> String {
    let (dir, ext) = rules_location(agents);
    // glob -> rule names, in first-seen order
    let mut by_glob: Vec<(&str, Vec<&str>)> = Vec::new();
    for rule in rules(features) {
        for glob in rule.paths {
            match by_glob.iter_mut().find(|(g, _)| g == glob) {
                Some((_, names)) => names.push(rule.name),
                None => by_glob.push((glob, vec![rule.name])),
            }
        }
    }
    // Merge globs that map to the same guides onto one line.
    let mut lines: Vec<(Vec<&str>, Vec<&str>)> = Vec::new();
    for (glob, names) in by_glob {
        match lines.iter_mut().find(|(_, n)| *n == names) {
            Some((globs, _)) => globs.push(glob),
            None => lines.push((vec![glob], names)),
        }
    }

    let auto: Vec<&str> = [Agent::Claude, Agent::Cursor]
        .into_iter()
        .filter(|a| has(agents, *a))
        .map(Agent::label)
        .collect();
    let mut md = format!("Detailed guides live in `{dir}/`");
    if has(agents, Agent::Claude) && has(agents, Agent::Cursor) {
        md.push_str(" (Cursor: same guides as `.cursor/rules/*.mdc`)");
    }
    if auto.is_empty() {
        md.push_str(". Before editing, read the guides for the files you touch:\n\n");
    } else {
        md.push_str(&format!(
            ". {} load{} them automatically for matching files; other agents should read the matching guides before editing:\n\n",
            auto.join(" and "),
            if auto.len() == 1 { "s" } else { "" }
        ));
    }
    for (globs, names) in lines {
        let globs: Vec<String> = globs.iter().map(|g| format!("`{g}`")).collect();
        let files: Vec<String> = names.iter().map(|n| format!("`{n}.{ext}`")).collect();
        md.push_str(&format!("- {}: {}\n", globs.join(", "), files.join(", ")));
    }
    md
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

/// A bundled skill (`<dir>/<name>/SKILL.md`). Content carries a
/// `# seite-skill-version: N` line that `seite upgrade` compares.
#[derive(Debug)]
pub struct Skill {
    pub name: &'static str,
    pub content: &'static str,
    pub needs: Needs,
}

pub const SKILLS: &[Skill] = &[
    Skill {
        name: "landing-page",
        content: include_str!("../scaffold/skill-landing-page.md"),
        needs: Needs::Pages,
    },
    Skill {
        name: "theme-builder",
        content: include_str!("../scaffold/skill-theme-builder.md"),
        needs: Needs::Always,
    },
    Skill {
        name: "brand-identity",
        content: include_str!("../scaffold/skill-brand-identity.md"),
        needs: Needs::Always,
    },
];

/// The skills that apply to a site with `features`.
pub fn skills(features: SiteFeatures) -> impl Iterator<Item = &'static Skill> {
    SKILLS.iter().filter(move |s| s.needs.applies(features))
}

/// Look up a skill by name.
pub fn skill(name: &str) -> Option<&'static Skill> {
    SKILLS.iter().find(|s| s.name == name)
}

/// Directory for Claude Code skills.
pub const CLAUDE_SKILLS_DIR: &str = ".claude/skills";
/// Shared skills directory read by Codex, Cursor, and OpenCode.
pub const SHARED_SKILLS_DIR: &str = ".agents/skills";

// ---------------------------------------------------------------------------
// MCP server + agent settings
// ---------------------------------------------------------------------------

/// The MCP server name, and the command + args that start it.
pub const MCP_SERVER: &str = "seite";
const MCP_COMMAND: &str = "seite";
const MCP_ARGS: &[&str] = &["mcp"];

/// The `seite` entry for `mcpServers` in `.mcp.json` / `.cursor/mcp.json`.
pub fn mcp_server_entry() -> serde_json::Value {
    serde_json::json!({ "command": MCP_COMMAND, "args": MCP_ARGS })
}

/// `{"seite": {...}}` — the `mcpServers` block with just the seite server.
pub fn mcp_server_block() -> serde_json::Value {
    serde_json::json!({ MCP_SERVER: mcp_server_entry() })
}

/// `.mcp.json` (Claude Code) and `.cursor/mcp.json` (Cursor) share this shape.
pub fn mcp_json() -> serde_json::Value {
    serde_json::json!({ "mcpServers": mcp_server_block() })
}

/// Permission rules pre-approved in a new site's `.claude/settings.json`.
///
/// `mcp__seite` allows every tool of the `seite` MCP server (Claude Code's
/// server-level MCP permission rule).
pub const CLAUDE_ALLOWED_TOOLS: &[&str] = &[
    "Read",
    "Write(content/**)",
    "Write(templates/**)",
    "Write(static/**)",
    "Write(data/**)",
    "Edit(content/**)",
    "Edit(templates/**)",
    "Edit(static/**)",
    "Edit(data/**)",
    "Edit(seite.toml)",
    "Bash(seite build:*)",
    "Bash(seite build)",
    "Bash(seite new:*)",
    "Bash(seite serve:*)",
    "Bash(seite theme:*)",
    "Glob",
    "Grep",
    "WebSearch",
    "mcp__seite",
];

/// Allow rules that `seite upgrade` adds to existing projects' settings
/// (introduced after the original settings template).
pub const CLAUDE_ALLOWED_TOOLS_UPGRADE: &[&str] =
    &["mcp__seite", "Edit(static/**)", "Edit(seite.toml)"];

/// Paths the agent may edit without asking (Claude `Write/Edit(...)`,
/// OpenCode `edit` rules).
const EDITABLE_PATHS: &[&str] = &["content/", "templates/", "static/", "data/"];
/// `seite` subcommands the agent may run without asking.
const ALLOWED_COMMANDS: &[&str] = &["seite build", "seite new", "seite serve", "seite theme"];

/// `.claude/settings.json` for a new site: permissions plus
/// `enabledMcpjsonServers`, which pre-approves the `seite` server declared in
/// `.mcp.json`. (Claude Code ignores `mcpServers` in settings.json.)
pub fn claude_settings() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json.schemastore.org/claude-code-settings.json",
        "permissions": {
            "allow": CLAUDE_ALLOWED_TOOLS,
            "deny": ["Read(.env)", "Read(.env.*)"]
        },
        "enabledMcpjsonServers": [MCP_SERVER]
    })
}

/// OpenCode's `permission` block, mirroring the Claude allowlist: reads are
/// allowed (but `.env` files denied), edits to site sources and the `seite`
/// build/new/serve/theme commands run without asking, anything else asks.
/// Patterns use OpenCode's wildcard syntax; the last matching rule wins.
pub fn opencode_permission() -> serde_json::Value {
    let mut edit = serde_json::Map::new();
    edit.insert("*".into(), "ask".into());
    for dir in EDITABLE_PATHS {
        edit.insert(format!("{dir}*"), "allow".into());
    }
    edit.insert("seite.toml".into(), "allow".into());

    let mut bash = serde_json::Map::new();
    bash.insert("*".into(), "ask".into());
    for cmd in ALLOWED_COMMANDS {
        bash.insert(format!("{cmd}*"), "allow".into());
    }

    serde_json::json!({
        "read": { "*": "allow", "*.env": "deny", "*.env.*": "deny" },
        "edit": edit,
        "bash": bash,
        "glob": "allow",
        "grep": "allow",
        "websearch": "allow",
        "webfetch": "ask"
    })
}

/// Allow rules for the Cursor CLI's project `.cursor/cli.json`: seite's MCP
/// tools, the `seite` command, and writes to site sources. Cursor matches
/// `Shell(...)` on the command's first token, so this allows every `seite`
/// subcommand (Cursor still asks before anything not listed here).
pub const CURSOR_ALLOW: &[&str] = &[
    "Mcp(seite:*)",
    "Shell(seite)",
    "Write(content/**)",
    "Write(templates/**)",
    "Write(static/**)",
    "Write(data/**)",
    "Write(seite.toml)",
];

/// `.cursor/cli.json` for a new site (Cursor CLI permissions).
pub fn cursor_cli_json() -> serde_json::Value {
    serde_json::json!({
        "permissions": {
            "allow": CURSOR_ALLOW,
            "deny": ["Read(.env)", "Read(.env.*)"]
        }
    })
}

/// The `mcp.seite` entry in `opencode.json`.
pub fn opencode_mcp_entry() -> serde_json::Value {
    let mut command = vec![MCP_COMMAND];
    command.extend_from_slice(MCP_ARGS);
    serde_json::json!({ "type": "local", "command": command, "enabled": true })
}

/// `opencode.json` for a new site.
pub fn opencode_json() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "mcp": { MCP_SERVER: opencode_mcp_entry() },
        "permission": opencode_permission()
    })
}

/// Leading comment for `.codex/config.toml` explaining the trust step.
const CODEX_HEADER: &str = "\
# Codex CLI project config for this seite site.
#
# Codex only loads a project's .codex/config.toml once you trust the project:
# answer \"trust\" when Codex asks on first run in this directory (this records
# `[projects.\"<path>\"] trust_level = \"trusted\"` in ~/.codex/config.toml).
# After that, `codex mcp list` (or /mcp inside Codex) shows the seite server.
";

/// Add `[mcp_servers.seite]` to a Codex config, preserving comments and every
/// other table. `Ok(None)` when the server is already declared (or
/// `mcp_servers` isn't a table we can extend); `Err` when the TOML is invalid.
pub fn merge_codex_config(existing: &str) -> Result<Option<String>, toml_edit::TomlError> {
    use toml_edit::{Array, DocumentMut, InlineTable, Item, Value};

    let mut doc: DocumentMut = existing.parse()?;
    match doc.get_mut("mcp_servers") {
        // No servers yet, or a `[mcp_servers]` / `[mcp_servers.x]` table:
        // append `[mcp_servers.seite]` at the end so the existing text —
        // comments, ordering, formatting — is kept byte for byte.
        None => {}
        Some(Item::Table(table)) => {
            if table.contains_key(MCP_SERVER) {
                return Ok(None);
            }
        }
        // `mcp_servers = { … }`: a header can't extend an inline table, so
        // add the entry inside it.
        Some(Item::Value(Value::InlineTable(table))) => {
            if table.contains_key(MCP_SERVER) {
                return Ok(None);
            }
            let mut seite = InlineTable::new();
            seite.insert("command", MCP_COMMAND.into());
            let args: Array = MCP_ARGS.iter().copied().collect();
            seite.insert("args", Value::Array(args));
            seite.insert("default_tools_approval_mode", "approve".into());
            table.insert(MCP_SERVER, Value::InlineTable(seite));
            return Ok(Some(doc.to_string()));
        }
        Some(_) => return Ok(None),
    }

    let mut merged = existing.trim_end().to_string();
    if !merged.is_empty() {
        merged.push_str("\n\n");
    }
    merged.push_str(&codex_server_table());
    // Guard against edge cases (e.g. dotted-key definitions) where appending
    // a header would not be valid TOML.
    merged.parse::<DocumentMut>()?;
    Ok(Some(merged))
}

/// `[mcp_servers.seite]` as TOML text.
fn codex_server_table() -> String {
    let args = MCP_ARGS
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(", ");
    // `approve` runs seite's tools without a prompt, like `mcp__seite` in
    // Claude's allowlist; without it `codex exec` refuses the write tools.
    format!(
        "[mcp_servers.{MCP_SERVER}]\ncommand = \"{MCP_COMMAND}\"\nargs = [{args}]\ndefault_tools_approval_mode = \"approve\"\n"
    )
}

/// `.codex/config.toml` for a new site.
pub fn codex_config_toml() -> String {
    format!("{CODEX_HEADER}\n{}", codex_server_table())
}

pub(crate) fn pretty_json(value: &serde_json::Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(value).unwrap_or_default()
    )
}

// ---------------------------------------------------------------------------
// AGENTS.md sections
// ---------------------------------------------------------------------------

/// Marker-delimited block in AGENTS.md listing each agent's MCP config.
pub const MCP_SETUP_BLOCK: &str = "agent-setup";
/// Marker-delimited block in AGENTS.md indexing the rules files.
pub const RULES_INDEX_BLOCK: &str = "context-rules";

pub fn block_start(id: &str) -> String {
    format!("<!-- seite:{id} -->")
}

pub fn block_end(id: &str) -> String {
    format!("<!-- /seite:{id} -->")
}

/// Wrap `body` in the start/end markers for block `id`.
pub fn wrap_block(id: &str, body: &str) -> String {
    format!(
        "{}\n{}\n{}\n",
        block_start(id),
        body.trim_end(),
        block_end(id)
    )
}

/// Replace the marked block `id` in `content` with `body`. `None` when the
/// markers are missing; `Some(content)` (possibly unchanged) otherwise.
pub fn replace_block(content: &str, id: &str, body: &str) -> Option<String> {
    let start = block_start(id);
    let end = block_end(id);
    let s = content.find(&start)?;
    let e = content[s..].find(&end)? + s + end.len();
    let mut tail = &content[e..];
    // wrap_block ends with a newline; swallow the one after the old end marker.
    if let Some(rest) = tail.strip_prefix('\n') {
        tail = rest;
    }
    Some(format!("{}{}{}", &content[..s], wrap_block(id, body), tail))
}

/// The per-agent MCP config table for AGENTS.md (selected agents only).
pub fn mcp_setup_table(agents: &[Agent]) -> String {
    let mut md = String::from("| Agent | MCP config | One-time step |\n|---|---|---|\n");
    for agent in agents {
        let (config, step) = match agent {
            Agent::Claude => (
                "`.mcp.json`",
                "Open the project once interactively and accept the workspace trust prompt — until then Claude Code ignores `.claude/settings.json` permissions; approve the server if asked (`/mcp`)",
            ),
            Agent::Codex => (
                "`.codex/config.toml`",
                "Trust the project when Codex asks (untrusted projects ignore the file, including its tool auto-approval); check with `/mcp`",
            ),
            Agent::Cursor => (
                "`.cursor/mcp.json` (+ `.cursor/cli.json` permissions)",
                "Approve the server in Cursor's MCP settings or run `cursor-agent mcp enable seite`; the CLI also needs workspace trust (`--trust` or answer the prompt)",
            ),
            Agent::Opencode => ("`opencode.json`", "None — it starts automatically"),
        };
        md.push_str(&format!("| {} | {config} | {step} |\n", agent.label()));
    }
    md
}

// ---------------------------------------------------------------------------
// Fresh-project plan (seite init)
// ---------------------------------------------------------------------------

/// A file `seite init` writes, relative to the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: String,
    pub content: String,
}

fn file(path: impl Into<String>, content: impl Into<String>) -> PlannedFile {
    PlannedFile {
        path: path.into(),
        content: content.into(),
    }
}

/// Every harness file a new site gets for `agents` (AGENTS.md itself is
/// generated by `cli::init`).
pub fn plan(agents: &[Agent], features: SiteFeatures) -> Vec<PlannedFile> {
    let mut files = Vec::new();

    if has(agents, Agent::Claude) {
        files.push(file(
            "CLAUDE.md",
            crate::cli::agent_instructions::CLAUDE_SHIM,
        ));
        files.push(file(
            ".claude/settings.json",
            pretty_json(&claude_settings()),
        ));
        files.push(file(".mcp.json", pretty_json(&mcp_json())));
        for s in skills(features) {
            files.push(file(
                format!("{CLAUDE_SKILLS_DIR}/{}/SKILL.md", s.name),
                s.content,
            ));
        }
        for r in rules(features) {
            files.push(file(format!(".claude/rules/{}.md", r.name), claude_rule(r)));
        }
    }
    if has(agents, Agent::Cursor) {
        files.push(file(".cursor/mcp.json", pretty_json(&mcp_json())));
        files.push(file(".cursor/cli.json", pretty_json(&cursor_cli_json())));
        for r in rules(features) {
            files.push(file(
                format!(".cursor/rules/{}.mdc", r.name),
                cursor_rule(r),
            ));
        }
    }
    if has(agents, Agent::Codex) {
        files.push(file(".codex/config.toml", codex_config_toml()));
    }
    if has(agents, Agent::Opencode) {
        files.push(file("opencode.json", pretty_json(&opencode_json())));
    }
    if uses_shared_skills(agents) {
        for s in skills(features) {
            files.push(file(
                format!("{SHARED_SKILLS_DIR}/{}/SKILL.md", s.name),
                s.content,
            ));
        }
    }
    if uses_shared_rules(agents) {
        for r in rules(features) {
            files.push(file(format!(".agents/rules/{}.md", r.name), claude_rule(r)));
        }
    }
    files
}

/// Write [`plan`] under `root`.
pub fn write_plan(root: &Path, agents: &[Agent], features: SiteFeatures) -> std::io::Result<()> {
    for f in plan(agents, features) {
        let path = root.join(&f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, f.content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_FEATURES: SiteFeatures = SiteFeatures {
        pages: true,
        contact: true,
        trust: true,
    };

    #[test]
    fn parse_agent_list_dedupes_and_orders() {
        assert_eq!(
            parse_agent_list("cursor, Claude,cursor").unwrap(),
            vec![Agent::Claude, Agent::Cursor]
        );
        assert_eq!(parse_agent_list("all").unwrap(), Agent::ALL.to_vec());
    }

    #[test]
    fn parse_agent_list_rejects_unknown_with_hint() {
        let err = parse_agent_list("claude,opencod").unwrap_err().to_string();
        assert!(err.contains("unknown agent 'opencod'"), "{err}");
        assert!(err.contains("did you mean 'opencode'"), "{err}");
        assert!(parse_agent_list(" , ").is_err());
    }

    #[test]
    fn from_ids_ignores_unknown() {
        let ids = vec!["codex".to_string(), "aider".to_string(), "claude".into()];
        assert_eq!(from_ids(&ids), vec![Agent::Claude, Agent::Codex]);
        assert_eq!(from_ids(&to_ids(&Agent::ALL)), Agent::ALL.to_vec());
    }

    #[test]
    fn cursor_rule_maps_paths_to_globs() {
        let rule = rule("i18n").unwrap();
        let mdc = cursor_rule(rule);
        assert!(
            mdc.starts_with("---\ndescription: \"Multi-language"),
            "{mdc}"
        );
        assert!(mdc.contains("\nglobs: \"content/**, templates/**, data/i18n/**\"\n"));
        assert!(mdc.contains("\nalwaysApply: false\n---\n## Multi-language Support"));
        assert!(!mdc.contains("paths:"));
    }

    #[test]
    fn cursor_rule_frontmatter_is_valid_yaml() {
        for rule in RULES {
            let mdc = cursor_rule(rule);
            let fm = mdc
                .strip_prefix("---\n")
                .and_then(|rest| rest.split_once("\n---\n"))
                .map(|(fm, _)| fm)
                .unwrap();
            let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(fm).unwrap();
            assert_eq!(yaml["description"].as_str(), Some(rule.description));
            assert_eq!(yaml["globs"].as_str(), Some(rule.paths.join(", ").as_str()));
            assert_eq!(yaml["alwaysApply"].as_bool(), Some(false));
        }
    }

    #[test]
    fn claude_rule_keeps_paths_frontmatter() {
        let out = claude_rule(rule("data-files").unwrap());
        assert!(out.starts_with("---\npaths:\n  - \"data/**\"\n---\n## Data Files"));
    }

    #[test]
    fn conditional_rules_and_skills_follow_features() {
        let none = SiteFeatures::default();
        let names: Vec<_> = rules(none).map(|r| r.name).collect();
        assert!(!names.contains(&"contact-form") && !names.contains(&"trust-center"));
        assert!(names.contains(&"private-collections"));
        assert_eq!(rules(ALL_FEATURES).count(), RULES.len());
        assert!(skills(none).all(|s| s.name != "landing-page"));
        assert_eq!(skills(ALL_FEATURES).count(), SKILLS.len());
    }

    #[test]
    fn cursor_cli_json_allows_seite_tools_and_denies_env() {
        let json = cursor_cli_json();
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|v| v == "Mcp(seite:*)"));
        assert!(allow.iter().any(|v| v == "Shell(seite)"));
        let deny = json["permissions"]["deny"].as_array().unwrap();
        assert!(deny.iter().any(|v| v == "Read(.env)"));
    }

    #[test]
    fn opencode_json_shape() {
        let json = opencode_json();
        assert_eq!(json["$schema"], "https://opencode.ai/config.json");
        assert_eq!(json["mcp"]["seite"]["type"], "local");
        assert_eq!(
            json["mcp"]["seite"]["command"],
            serde_json::json!(["seite", "mcp"])
        );
        assert_eq!(json["mcp"]["seite"]["enabled"], true);
        let perm = &json["permission"];
        assert_eq!(perm["bash"]["*"], "ask");
        assert_eq!(perm["bash"]["seite build*"], "allow");
        assert_eq!(perm["bash"]["seite theme*"], "allow");
        assert_eq!(perm["edit"]["*"], "ask");
        assert_eq!(perm["edit"]["content/*"], "allow");
        assert_eq!(perm["edit"]["seite.toml"], "allow");
        assert_eq!(perm["read"]["*.env"], "deny");
        assert_eq!(perm["read"]["*"], "allow");
        // OpenCode applies the last matching rule, so each catch-all "*" must
        // come first in the written file.
        let text = pretty_json(&json);
        for key in ["read", "edit", "bash"] {
            let block = &text[text.find(&format!("\"{key}\": {{")).unwrap()..];
            let first_rule = block.lines().nth(1).unwrap().trim();
            assert!(first_rule.starts_with("\"*\":"), "{key}: {first_rule}");
        }
    }

    #[test]
    fn codex_config_declares_server_with_trust_comment() {
        let toml_str = codex_config_toml();
        assert!(
            toml_str.starts_with("# Codex CLI project config"),
            "{toml_str}"
        );
        assert!(toml_str.contains("trust"));
        assert!(toml_str.contains("[mcp_servers.seite]"));
        assert!(!toml_str.contains("[mcp_servers]\n"), "{toml_str}");
        let parsed: toml::Value = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            parsed["mcp_servers"]["seite"]["command"].as_str(),
            Some("seite")
        );
        assert_eq!(
            parsed["mcp_servers"]["seite"]["args"].as_array().unwrap()[0].as_str(),
            Some("mcp")
        );
    }

    #[test]
    fn merge_codex_config_preserves_existing_content() {
        let existing =
            "# my notes\nmodel = \"o3\" # inline\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        let merged = merge_codex_config(existing).unwrap().unwrap();
        assert!(merged.starts_with(existing), "{merged}");
        assert!(merged.contains("[mcp_servers.seite]"));
        // Idempotent
        assert!(merge_codex_config(&merged).unwrap().is_none());
        // Inline mcp_servers table is extended in place
        let inline = "mcp_servers = { other = { command = \"x\" } }\n";
        let merged = merge_codex_config(inline).unwrap().unwrap();
        let parsed: toml::Value = toml::from_str(&merged).unwrap();
        assert!(parsed["mcp_servers"].get("other").is_some());
        assert_eq!(
            parsed["mcp_servers"]["seite"]["command"].as_str(),
            Some("seite")
        );
        // Invalid TOML is an error, not a rewrite
        assert!(merge_codex_config("[broken").is_err());
    }

    #[test]
    fn replace_block_swaps_only_marked_content() {
        let doc = format!("# T\n\n{}after\n", wrap_block("x", "old"));
        let out = replace_block(&doc, "x", "new").unwrap();
        assert_eq!(out, format!("# T\n\n{}after\n", wrap_block("x", "new")));
        assert_eq!(replace_block(&out, "x", "new").unwrap(), out);
        assert!(replace_block("no markers", "x", "new").is_none());
    }

    #[test]
    fn rules_location_prefers_claude_then_cursor() {
        assert_eq!(rules_location(&Agent::ALL).0, ".claude/rules");
        assert_eq!(
            rules_location(&[Agent::Codex, Agent::Cursor]),
            (".cursor/rules", "mdc")
        );
        assert_eq!(rules_location(&[Agent::Codex]).0, ".agents/rules");
    }

    #[test]
    fn rules_index_groups_globs() {
        let idx = rules_index(ALL_FEATURES, &Agent::ALL);
        assert!(idx.contains("`.claude/rules/`"));
        assert!(idx.contains("Claude Code and Cursor load them automatically"));
        assert!(idx.contains(
            "- `seite.toml`: `config-reference.md`, `private-collections.md`, `contact-form.md`"
        ));
        assert!(idx.contains("- `content/trust/**`, `data/trust/**`: `trust-center.md`"));
        let codex_only = rules_index(SiteFeatures::default(), &[Agent::Codex]);
        assert!(codex_only.contains("`.agents/rules/`"));
        assert!(codex_only.contains("read the guides for the files you touch"));
        assert!(!codex_only.contains("contact-form"));
    }

    #[test]
    fn mcp_setup_table_lists_only_selected_agents() {
        let table = mcp_setup_table(&[Agent::Codex, Agent::Opencode]);
        assert!(table.contains("`.codex/config.toml`") && table.contains("`opencode.json`"));
        assert!(!table.contains(".mcp.json") && !table.contains("cursor"));
    }

    #[test]
    fn plan_subset_writes_only_selected_agent_files() {
        let paths: Vec<String> = plan(&[Agent::Claude], ALL_FEATURES)
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert!(paths.contains(&".mcp.json".to_string()));
        assert!(paths.iter().all(|p| !p.starts_with(".cursor")
            && !p.starts_with(".codex")
            && !p.starts_with(".agents")
            && p != "opencode.json"));

        let paths: Vec<String> = plan(&[Agent::Codex], SiteFeatures::default())
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert!(paths.contains(&".codex/config.toml".to_string()));
        assert!(paths.contains(&".agents/skills/theme-builder/SKILL.md".to_string()));
        assert!(paths.contains(&".agents/rules/templates.md".to_string()));
        assert!(paths
            .iter()
            .all(|p| !p.starts_with(".claude") && p != "CLAUDE.md"));
    }
}
