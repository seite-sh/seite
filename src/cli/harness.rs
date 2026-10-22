//! Coding-agent harness files for generated sites.
//!
//! A seite site can be set up for several coding agents at once: Claude Code,
//! OpenAI Codex CLI, OpenCode, Cursor (editor + `cursor-agent`), and Gemini
//! CLI. Each one reads project context from different places, so this module
//! declares every rules file, the `seite` skill (with its reference files),
//! and the optional MCP server entry **once**, and every per-agent difference
//! as a row of [`PROVIDERS`] (with the reason and source for each quirk), then
//! renders them into each agent's format:
//!
//! | Agent    | MCP config (`--mcp`)    | Path-scoped rules     | Skills            | `/seite` |
//! |----------|-------------------------|-----------------------|-------------------|----------|
//! | claude   | `.mcp.json`             | `.claude/rules/*.md`  | `.claude/skills/` | skill    |
//! | cursor   | `.cursor/mcp.json`      | `.cursor/rules/*.mdc` | `.agents/skills/` | skill    |
//! | codex    | `.codex/config.toml`    | (index in AGENTS.md)  | `.agents/skills/` | `$seite` |
//! | opencode | `opencode.json`         | (index in AGENTS.md)  | `.agents/skills/` | `.opencode/commands/seite.md` |
//! | gemini   | `.gemini/settings.json` | (index in AGENTS.md)  | `.agents/skills/` | `.gemini/commands/seite.toml` |
//!
//! Coding agents use the `seite` CLI; the MCP server is only wired in with
//! `seite init --mcp` / `seite upgrade --mcp` (it is aimed at chat apps
//! without a shell). `seite init` writes [`plan`]; `seite upgrade` merges the
//! same content into existing projects (see `cli::upgrade`).

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
    Gemini,
}

impl Agent {
    /// Every supported agent, in canonical order.
    pub const ALL: [Agent; 5] = [
        Agent::Claude,
        Agent::Codex,
        Agent::Opencode,
        Agent::Cursor,
        Agent::Gemini,
    ];

    /// This agent's row in [`PROVIDERS`].
    pub fn spec(self) -> &'static AgentSpec {
        match self {
            Agent::Claude => &PROVIDERS[0],
            Agent::Codex => &PROVIDERS[1],
            Agent::Opencode => &PROVIDERS[2],
            Agent::Cursor => &PROVIDERS[3],
            Agent::Gemini => &PROVIDERS[4],
        }
    }

    /// The identifier used by `--agents` and stored in `.seite/config.json`.
    pub fn id(self) -> &'static str {
        self.spec().id
    }

    /// Human-readable product name.
    pub fn label(self) -> &'static str {
        self.spec().label
    }

    pub fn from_id(id: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.id() == id)
    }
}

// ---------------------------------------------------------------------------
// Provider table: every per-agent difference, as data
// ---------------------------------------------------------------------------

/// How an agent's MCP config file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpFormat {
    /// `{"mcpServers": {"seite": {"command", "args"}}}` ([`mcp_json`]).
    McpServersJson,
    /// A `[mcp_servers.seite]` TOML table ([`codex_config_toml`]).
    CodexToml,
    /// `mcp.seite` (`type: local`, `command: [...]`) plus the `permission`
    /// block ([`opencode_json`]).
    OpencodeJson,
}

impl McpFormat {
    /// The config file a new site gets.
    pub fn render(self) -> String {
        match self {
            McpFormat::McpServersJson => pretty_json(&mcp_json()),
            McpFormat::CodexToml => codex_config_toml(),
            McpFormat::OpencodeJson => pretty_json(&opencode_json(true)),
        }
    }
}

/// How an agent's native path-scoped rules are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesFormat {
    /// Markdown with a `paths:` list in the frontmatter ([`claude_rule`]).
    ClaudePaths,
    /// Cursor MDC: `description` / `globs` / `alwaysApply` ([`cursor_rule`]).
    CursorMdc,
}

impl RulesFormat {
    pub fn render(self, rule: &Rule) -> String {
        match self {
            RulesFormat::ClaudePaths => claude_rule(rule),
            RulesFormat::CursorMdc => cursor_rule(rule),
        }
    }
}

/// Where an agent loads path-scoped rules from natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulesDir {
    pub dir: &'static str,
    pub ext: &'static str,
    pub format: RulesFormat,
}

/// The neutral rules copy for sites with no agent that has native rules
/// (Codex/OpenCode read these on demand via the AGENTS.md index).
pub const SHARED_RULES: RulesDir = RulesDir {
    dir: ".agents/rules",
    ext: "md",
    format: RulesFormat::ClaudePaths,
};

/// Which SKILL.md frontmatter keys a skills directory may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillFrontmatter {
    /// `name` + `description` only (the Agent Skills spec subset every agent
    /// accepts). The seite version stays a YAML comment
    /// (`# seite-skill-version: N`), which no validator sees as a key.
    Portable,
    /// Portable plus Claude Code's `argument-hint` (shown in the `/` menu).
    ClaudeCode,
}

/// Slash-command wrappers that route `/<skill> <args>` to a skill, for agents
/// without a native command→skill bridge.
#[derive(Debug, Clone, Copy)]
pub struct CommandFiles {
    /// Directory the agent reads command files from.
    pub dir: &'static str,
    /// File extension of a command file (`md`, `toml`).
    pub ext: &'static str,
    /// Renders `<dir>/<skill>.<ext>` for a skill that has a command table
    /// (`None` for skills that don't get a wrapper).
    pub render: fn(&Skill) -> Option<String>,
}

/// Builds the JSON of a config file a new site gets (`true`: with the seite
/// MCP server wired in).
pub type JsonTemplate = fn(bool) -> serde_json::Value;

/// Everything seite needs to know to set a site up for one coding agent.
///
/// Each quirk carries a short "why" and its source, so the table can be
/// re-checked when an agent changes its conventions.
#[derive(Debug)]
pub struct AgentSpec {
    pub agent: Agent,
    /// `--agents` id, stored in `.seite/config.json`.
    pub id: &'static str,
    pub label: &'static str,
    /// The agent's own project config files (permissions, hooks), as shown
    /// in the AGENTS.md setup table.
    pub config_display: &'static str,
    /// Project MCP config file, relative to the site root (only written
    /// with `--mcp`).
    pub mcp_config: &'static str,
    pub mcp_format: McpFormat,
    /// How the MCP config appears in the AGENTS.md setup table.
    pub mcp_config_display: &'static str,
    /// Native path-scoped rules, if the agent has them. Agents without read
    /// the guides on demand via the AGENTS.md rules index.
    pub rules: Option<RulesDir>,
    /// The skills directory seite writes for this agent.
    pub skills_dir: &'static str,
    pub skill_frontmatter: SkillFrontmatter,
    /// Project permissions file and the JSON a new site gets.
    pub permissions_file: Option<(&'static str, JsonTemplate)>,
    /// Slash-command wrappers for skills, if the agent needs them.
    pub command_files: Option<CommandFiles>,
    /// A per-agent instructions file that only imports AGENTS.md.
    pub instructions_shim: Option<&'static str>,
    /// One-time step before the project config (permissions, hooks) takes
    /// effect (shown in the AGENTS.md setup table).
    pub setup_step: &'static str,
    /// The extra one-time step for the MCP server, when it is wired in.
    pub mcp_step: &'static str,
    /// Other quirks worth knowing.
    pub notes: &'static [&'static str],
    /// CLI names on `PATH` that mean the agent is installed.
    pub binaries: &'static [&'static str],
    /// Config directories under `$HOME` that mean the agent has been used.
    pub home_dirs: &'static [&'static str],
}

/// Directory for Claude Code skills.
pub const CLAUDE_SKILLS_DIR: &str = ".claude/skills";
/// Shared skills directory read by Codex, Cursor, and OpenCode.
pub const SHARED_SKILLS_DIR: &str = ".agents/skills";

/// The provider table, in [`Agent::ALL`] order.
pub const PROVIDERS: [AgentSpec; 5] = [
    AgentSpec {
        agent: Agent::Claude,
        id: "claude",
        label: "Claude Code",
        config_display: "`.claude/settings.json`, `.claude/launch.json`",
        // Claude Code reads project MCP servers only from `.mcp.json`;
        // `mcpServers` in `.claude/settings.json` is never loaded.
        // https://code.claude.com/docs/en/mcp
        mcp_config: ".mcp.json",
        mcp_format: McpFormat::McpServersJson,
        mcp_config_display: "`.mcp.json`",
        // https://code.claude.com/docs/en/memory (path-scoped `.claude/rules`)
        rules: Some(RulesDir {
            dir: ".claude/rules",
            ext: "md",
            format: RulesFormat::ClaudePaths,
        }),
        skills_dir: CLAUDE_SKILLS_DIR,
        // Claude Code skills take `argument-hint` and are slash commands
        // already (`/seite check`), so no wrapper file.
        // https://code.claude.com/docs/en/skills
        skill_frontmatter: SkillFrontmatter::ClaudeCode,
        permissions_file: Some((".claude/settings.json", claude_settings_template)),
        command_files: None,
        // Claude Code reads AGENTS.md itself only from v2.1.277, only when no
        // CLAUDE.md exists, and not in every session; a CLAUDE.md that imports
        // it works everywhere. https://code.claude.com/docs/en/memory#agents-md
        instructions_shim: Some("CLAUDE.md"),
        // Claude ignores project permissions (and hooks) until the workspace
        // is trusted (claude -p warns "Ignoring N permissions.allow entries
        // ... not trusted"). https://code.claude.com/docs/en/settings
        setup_step: "Accept the workspace trust prompt once (until then Claude Code ignores `.claude/settings.json`)",
        mcp_step: "approve the server with `/mcp` if asked",
        notes: &["`enabledMcpjsonServers` in .claude/settings.json pre-approves the .mcp.json server (with --mcp)."],
        binaries: &["claude"],
        home_dirs: &[".claude"],
    },
    AgentSpec {
        agent: Agent::Codex,
        id: "codex",
        label: "Codex CLI",
        config_display: "`.codex/hooks.json`",
        // https://developers.openai.com/codex/mcp
        mcp_config: ".codex/config.toml",
        mcp_format: McpFormat::CodexToml,
        mcp_config_display: "`.codex/config.toml`",
        // No path-scoped rules; Codex reads AGENTS.md.
        rules: None,
        // Codex discovers repo skills only in `.agents/skills` (cwd up to the
        // repo root); `$seite` or `/skills` invokes one.
        // https://learn.chatgpt.com/docs/build-skills
        skills_dir: SHARED_SKILLS_DIR,
        // Codex's skill validator is reported to reject unknown top-level
        // frontmatter keys, so impeccable moves `version` under `metadata`
        // for Codex (https://github.com/pbakaus/impeccable/blob/main/scripts/lib/transformers/providers.js).
        // codex-cli 0.154 still listed a probe skill with `argument-hint`,
        // but the shared directory stays on the spec subset every reader
        // accepts; seite's version is a YAML comment, not a key.
        skill_frontmatter: SkillFrontmatter::Portable,
        permissions_file: None,
        command_files: None,
        instructions_shim: None,
        // Codex loads project hooks and .codex/config.toml only once the
        // project is trusted, and asks to review new hooks in `/hooks`.
        // https://learn.chatgpt.com/docs/config-file/config-basic
        // https://learn.chatgpt.com/docs/hooks
        setup_step: "Trust the project when Codex asks, then approve the hooks once in `/hooks`",
        mcp_step: "check the server with `/mcp`",
        notes: &["`default_tools_approval_mode = \"approve\"` lets `codex exec` call seite's write tools (with --mcp)."],
        binaries: &["codex"],
        home_dirs: &[".codex"],
    },
    AgentSpec {
        agent: Agent::Opencode,
        id: "opencode",
        label: "OpenCode",
        config_display: "`opencode.json`, `.opencode/plugins/seite-check.js`",
        // https://opencode.ai/docs/mcp-servers
        mcp_config: "opencode.json",
        mcp_format: McpFormat::OpencodeJson,
        mcp_config_display: "`opencode.json`",
        rules: None,
        // OpenCode reads `.opencode/skills`, `.claude/skills`, and
        // `.agents/skills`, keeping only name/description/license/
        // compatibility/metadata. https://opencode.ai/docs/skills
        skills_dir: SHARED_SKILLS_DIR,
        skill_frontmatter: SkillFrontmatter::Portable,
        // Permissions live in opencode.json's `permission` block.
        permissions_file: Some(("opencode.json", opencode_json_template)),
        // OpenCode has no slash-command→skill bridge (skills load through its
        // `skill` tool), so `/seite` is a command file whose body calls the
        // skill with $ARGUMENTS. https://opencode.ai/docs/commands — impeccable
        // ships the same wrapper in .opencode/commands/impeccable.md.
        command_files: Some(CommandFiles {
            dir: ".opencode/commands",
            ext: "md",
            render: opencode_command,
        }),
        instructions_shim: None,
        setup_step: "None",
        mcp_step: "",
        notes: &["With Claude Code also selected, OpenCode finds each skill in both .claude/skills and .agents/skills."],
        binaries: &["opencode"],
        home_dirs: &[".config/opencode"],
    },
    AgentSpec {
        agent: Agent::Cursor,
        id: "cursor",
        label: "Cursor",
        config_display: "`.cursor/cli.json`, `.cursor/hooks.json`",
        // https://cursor.com/docs/context/mcp
        mcp_config: ".cursor/mcp.json",
        mcp_format: McpFormat::McpServersJson,
        mcp_config_display: "`.cursor/mcp.json`",
        // Cursor reads `.cursor/rules/*.mdc` and ignores `.claude/rules`.
        // https://cursor.com/docs/context/rules
        rules: Some(RulesDir {
            dir: ".cursor/rules",
            ext: "mdc",
            format: RulesFormat::CursorMdc,
        }),
        // Cursor loads `.agents/skills` natively and runs skills as `/name`;
        // its slash commands were folded into skills (`/migrate-to-skills`),
        // so no command file. https://cursor.com/docs/context/skills
        skills_dir: SHARED_SKILLS_DIR,
        skill_frontmatter: SkillFrontmatter::Portable,
        // https://cursor.com/docs/cli/reference/permissions
        permissions_file: Some((".cursor/cli.json", cursor_cli_json_template)),
        command_files: None,
        instructions_shim: None,
        setup_step: "Trust the workspace (`cursor-agent --trust` or answer the prompt)",
        mcp_step: "approve the server in Cursor's MCP settings or run `cursor-agent mcp enable seite`",
        notes: &["cursor-agent matches `Shell(...)` permissions on the command's first token."],
        // The Cursor CLI installs as `cursor-agent` (newer builds also as
        // `agent`); the editor adds a `cursor` shell command.
        binaries: &["cursor-agent", "cursor", "agent"],
        home_dirs: &[".cursor"],
    },
    AgentSpec {
        agent: Agent::Gemini,
        id: "gemini",
        label: "Gemini CLI",
        config_display: "`.gemini/settings.json`",
        // Project MCP servers and hooks both live in `.gemini/settings.json`
        // (`mcpServers.<name>` with `command`/`args`).
        // https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md
        mcp_config: ".gemini/settings.json",
        mcp_format: McpFormat::McpServersJson,
        mcp_config_display: "`.gemini/settings.json`",
        // No path-scoped rules; the AGENTS.md index points at `.agents/rules`.
        rules: None,
        // Workspace skills load from `.gemini/skills/` or its alias
        // `.agents/skills/` (on by default via `skills.enabled`); the model
        // activates them with its `activate_skill` tool.
        // https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/skills.md
        skills_dir: SHARED_SKILLS_DIR,
        skill_frontmatter: SkillFrontmatter::Portable,
        permissions_file: None,
        // Skills have no slash command of their own, so `/seite` is a custom
        // command (`.gemini/commands/<name>.toml`: `description` + `prompt`,
        // `{{args}}` replaced by what the user typed).
        // https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/custom-commands.md
        command_files: Some(CommandFiles {
            dir: ".gemini/commands",
            ext: "toml",
            render: gemini_command,
        }),
        // Gemini CLI reads GEMINI.md by default and supports `@file.md`
        // imports, so a one-line GEMINI.md imports AGENTS.md (the same shim
        // as Claude's CLAUDE.md). `context.fileName: ["AGENTS.md"]` in
        // settings.json would also work, but it is documented only in
        // docs/cli/gemini-md.md, not in the settings reference.
        // https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/gemini-md.md
        instructions_shim: Some("GEMINI.md"),
        // Project hooks are fingerprinted and need approval when new or
        // changed; stdio MCP servers stay disconnected until the folder is
        // trusted. https://github.com/google-gemini/gemini-cli/blob/main/docs/hooks/index.md
        setup_step: "Trust the folder (`gemini trust` or the prompt) and approve the project hooks when asked",
        mcp_step: "check the server with `/mcp`",
        notes: &["Hook timeouts are milliseconds in Gemini CLI (seconds elsewhere)."],
        binaries: &["gemini"],
        home_dirs: &[".gemini"],
    },
];

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

/// Agents that look installed on this machine: one of the agent's CLIs is on
/// `path` (a `PATH`-style list), or its config directory exists under `home`.
/// Returned in canonical order.
pub fn detect_installed_agents(path: Option<&std::ffi::OsStr>, home: Option<&Path>) -> Vec<Agent> {
    let dirs: Vec<std::path::PathBuf> = path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    let on_path = |bin: &str| {
        dirs.iter().any(|dir| {
            dir.join(bin).is_file()
                || (cfg!(windows)
                    && ["exe", "cmd"]
                        .iter()
                        .any(|ext| dir.join(format!("{bin}.{ext}")).is_file()))
        })
    };
    Agent::ALL
        .into_iter()
        .filter(|agent| {
            let spec = agent.spec();
            spec.binaries.iter().any(|b| on_path(b))
                || home.is_some_and(|h| spec.home_dirs.iter().any(|d| h.join(d).is_dir()))
        })
        .collect()
}

/// Preselection for the interactive agent picker: the detected agents, or
/// every agent when none is detected.
pub fn interactive_defaults(detected: &[Agent]) -> Vec<bool> {
    if detected.is_empty() {
        return vec![true; Agent::ALL.len()];
    }
    Agent::ALL.iter().map(|a| detected.contains(a)).collect()
}

/// Resolve the agent selection for `seite init`: the `--agents` flag, else an
/// interactive multi-select with the agents installed on this machine
/// preselected (all when none is found), else (not interactive) every agent —
/// so scripted runs stay deterministic.
pub fn resolve_init_agents(flag: Option<&str>) -> anyhow::Result<Vec<Agent>> {
    if let Some(list) = flag {
        return parse_agent_list(list);
    }
    let labels: Vec<&str> = Agent::ALL.iter().map(|a| a.label()).collect();
    let defaults = if prompt::is_interactive() {
        interactive_defaults(&detect_installed_agents(
            std::env::var_os("PATH").as_deref(),
            crate::platform::home_dir().as_deref(),
        ))
    } else {
        vec![true; Agent::ALL.len()]
    };
    let selected = prompt::multi_select(
        "Coding agents to set up (space toggles; detected agents preselected)",
        &labels,
        &defaults,
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
    agents
        .iter()
        .any(|a| a.spec().skills_dir == SHARED_SKILLS_DIR)
}

/// Selected agents' native rules directories, in canonical order.
fn native_rules(agents: &[Agent]) -> impl Iterator<Item = (Agent, RulesDir)> + '_ {
    Agent::ALL
        .into_iter()
        .filter(|a| has(agents, *a))
        .filter_map(|a| a.spec().rules.map(|r| (a, r)))
}

/// The rules directory AGENTS.md's index points at: the first selected
/// agent's native rules (Claude's `.claude/rules/`, else Cursor's
/// `.cursor/rules/`), else the neutral [`SHARED_RULES`] copy.
pub fn index_rules(agents: &[Agent]) -> RulesDir {
    native_rules(agents)
        .map(|(_, r)| r)
        .next()
        .unwrap_or(SHARED_RULES)
}

/// Where the rules files that AGENTS.md points at live, and their extension.
pub fn rules_location(agents: &[Agent]) -> (&'static str, &'static str) {
    let rules = index_rules(agents);
    (rules.dir, rules.ext)
}

/// Whether the neutral `.agents/rules/` copy is the index target.
pub fn uses_shared_rules(agents: &[Agent]) -> bool {
    index_rules(agents) == SHARED_RULES
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

    let auto: Vec<&str> = native_rules(agents).map(|(a, _)| a.label()).collect();
    let mut md = format!("Detailed guides live in `{dir}/`");
    for (agent, rules) in native_rules(agents).filter(|(_, r)| r.dir != dir) {
        md.push_str(&format!(
            " ({}: same guides as `{}/*.{}`)",
            agent.label(),
            rules.dir,
            rules.ext
        ));
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

/// A bundled skill (`<dir>/<name>/SKILL.md` plus `<dir>/<name>/reference/*.md`).
/// SKILL.md carries a `# seite-skill-version: N` line that `seite upgrade`
/// compares; the reference files are replaced together with it.
#[derive(Debug)]
pub struct Skill {
    pub name: &'static str,
    /// Portable SKILL.md (frontmatter: `name`, `description`, and the
    /// `# seite-skill-version` comment).
    pub content: &'static str,
    pub needs: Needs,
    /// `argument-hint` for agents whose skills take one
    /// ([`SkillFrontmatter::ClaudeCode`]). Skills with a hint are verb
    /// dispatchers and also get command wrappers ([`CommandFiles`]).
    pub argument_hint: Option<&'static str>,
    /// `reference/<name>.md` files: one workflow per command, read on demand
    /// so the always-loaded SKILL.md stays short. Identical for every agent.
    pub references: &'static [Reference],
}

/// A skill reference file, written as `<skill dir>/reference/<name>.md`.
#[derive(Debug)]
pub struct Reference {
    pub name: &'static str,
    pub content: &'static str,
}

macro_rules! reference {
    ($name:literal) => {
        Reference {
            name: $name,
            content: include_str!(concat!("../scaffold/skill-seite/reference/", $name, ".md")),
        }
    };
}

/// The `seite` skill's command references, in SKILL.md table order.
pub const SEITE_REFERENCES: &[Reference] = &[
    reference!("check"),
    reference!("new"),
    reference!("preview"),
    reference!("build"),
    reference!("deploy"),
    reference!("theme"),
    reference!("brand"),
    reference!("landing"),
    reference!("collection"),
    reference!("upgrade"),
    reference!("hooks"),
];

pub const SKILLS: &[Skill] = &[Skill {
    name: "seite",
    content: include_str!("../scaffold/skill-seite/SKILL.md"),
    needs: Needs::Always,
    argument_hint: Some(
        "[check | new <type> \"<title>\" | preview | build | deploy | theme [name|description] | brand | landing [page] | collection [preset] | upgrade | hooks [on|off|status]]",
    ),
    references: SEITE_REFERENCES,
}];

/// Every file of `skill` in skills directory `dir` (SKILL.md first), as
/// (relative path, content).
pub fn skill_files(dir: &str, skill: &Skill) -> Vec<(String, String)> {
    let mut files = vec![(
        format!("{dir}/{}/SKILL.md", skill.name),
        skill.render(skill_frontmatter_for(dir)),
    )];
    for r in skill.references {
        files.push((
            format!("{dir}/{}/reference/{}.md", skill.name, r.name),
            r.content.to_string(),
        ));
    }
    files
}

impl Skill {
    /// SKILL.md as written into a skills directory with `frontmatter` rules.
    pub fn render(&self, frontmatter: SkillFrontmatter) -> String {
        match (frontmatter, self.argument_hint) {
            (SkillFrontmatter::ClaudeCode, Some(hint)) => insert_after_description(
                self.content,
                &format!("argument-hint: {}", yaml_quote(hint)),
            ),
            _ => self.content.to_string(),
        }
    }

    /// The `description:` value from the frontmatter.
    pub fn description(&self) -> &'static str {
        frontmatter_lines(self.content)
            .find_map(|line| line.strip_prefix("description:"))
            .map(str::trim)
            .unwrap_or_default()
    }

    /// The `# seite-skill-version: N` value (0 when missing).
    pub fn version(&self) -> u32 {
        extract_version(self.content)
    }
}

/// How a skills directory's SKILL.md frontmatter is rendered: Claude's own
/// directory takes `argument-hint`; the shared `.agents/skills` stays portable
/// because Codex, Cursor, and OpenCode all read it.
pub fn skill_frontmatter_for(dir: &str) -> SkillFrontmatter {
    PROVIDERS
        .iter()
        .find(|p| p.skills_dir == dir)
        .map_or(SkillFrontmatter::Portable, |p| p.skill_frontmatter)
}

/// Extract the `# seite-skill-version: N` value from a skill or command file.
/// Returns 0 if not found.
pub fn extract_version(content: &str) -> u32 {
    content
        .lines()
        .filter_map(|line| line.trim().strip_prefix("# seite-skill-version:"))
        .find_map(|rest| rest.trim().parse().ok())
        .unwrap_or(0)
}

/// Lines between the opening and closing `---` of a frontmatter block.
fn frontmatter_lines(content: &str) -> impl Iterator<Item = &str> {
    content
        .strip_prefix("---\n")
        .unwrap_or_default()
        .lines()
        .take_while(|line| *line != "---")
}

/// `content` with `line` inserted after the frontmatter's `description:` line.
fn insert_after_description(content: &str, line: &str) -> String {
    let mut out = String::with_capacity(content.len() + line.len() + 1);
    let mut inserted = false;
    for l in content.split_inclusive('\n') {
        out.push_str(l);
        if !inserted && l.starts_with("description:") {
            out.push_str(line);
            out.push('\n');
            inserted = true;
        }
    }
    out
}

/// OpenCode `.opencode/commands/<skill>.md`: `/seite <args>` loads the skill
/// through OpenCode's `skill` tool and hands it the arguments. No `agent` or
/// `subtask`, so it runs in the current agent (a deploy confirmation must
/// reach the user).
fn opencode_command(skill: &Skill) -> Option<String> {
    skill.argument_hint?;
    Some(format!(
        "---\ndescription: {}\n# seite-skill-version: {}\n---\nLoad the `{name}` skill (call skill({{ name: \"{name}\" }})) and follow its Routing section and command table for: $ARGUMENTS\n",
        yaml_quote(skill.description()),
        skill.version(),
        name = skill.name,
    ))
}

/// Gemini CLI `.gemini/commands/<skill>.toml`: `/seite <args>` activates the
/// skill (Gemini loads it from `.agents/skills`) and hands it the arguments.
fn gemini_command(skill: &Skill) -> Option<String> {
    skill.argument_hint?;
    Some(format!(
        "# seite-skill-version: {}\ndescription = {}\nprompt = \"\"\"\nActivate the `{name}` skill (.agents/skills/{name}/SKILL.md) and follow its Routing section and command table for: {{{{args}}}}\n\"\"\"\n",
        skill.version(),
        toml_quote(skill.description()),
        name = skill.name,
    ))
}

fn toml_quote(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// A command wrapper file planned for `agents`: (relative path, content).
pub fn command_files(agents: &[Agent], features: SiteFeatures) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for agent in agents {
        let Some(cmds) = agent.spec().command_files else {
            continue;
        };
        for s in skills(features) {
            if let Some(content) = (cmds.render)(s) {
                files.push((format!("{}/{}.{}", cmds.dir, s.name, cmds.ext), content));
            }
        }
    }
    files
}

/// The skills that apply to a site with `features`.
pub fn skills(features: SiteFeatures) -> impl Iterator<Item = &'static Skill> {
    SKILLS.iter().filter(move |s| s.needs.applies(features))
}

/// Look up a skill by name.
pub fn skill(name: &str) -> Option<&'static Skill> {
    SKILLS.iter().find(|s| s.name == name)
}

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

/// Permission rules pre-approved in a new site's `.claude/settings.json`
/// (plus [`CLAUDE_MCP_ALLOW`] with `--mcp`).
///
/// `seite` commands are allowed one by one: the read-only and site-editing
/// ones run without a prompt, `seite deploy` (publishes) still asks.
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
    "Bash(seite check:*)",
    "Bash(seite check)",
    "Bash(seite new:*)",
    "Bash(seite serve:*)",
    "Bash(seite theme:*)",
    "Bash(seite context:*)",
    "Bash(seite context)",
    "Bash(seite docs:*)",
    "Bash(seite docs)",
    "Bash(seite page:*)",
    "Bash(seite search:*)",
    "Bash(seite stats:*)",
    "Bash(seite stats)",
    "Bash(seite templates:*)",
    "Bash(seite templates)",
    "Bash(seite frontmatter:*)",
    "Bash(seite explain:*)",
    "Bash(seite hooks:*)",
    "Bash(seite collection list)",
    "Glob",
    "Grep",
    "WebSearch",
];

/// `mcp__seite` allows every tool of the `seite` MCP server (Claude Code's
/// server-level MCP permission rule). Only with `--mcp`.
pub const CLAUDE_MCP_ALLOW: &str = "mcp__seite";

/// Allow rules that `seite upgrade` adds to existing projects' settings
/// (introduced after the original settings template).
pub const CLAUDE_ALLOWED_TOOLS_UPGRADE: &[&str] = &[
    "Edit(static/**)",
    "Edit(seite.toml)",
    "Bash(seite check:*)",
    "Bash(seite check)",
    "Bash(seite context:*)",
    "Bash(seite context)",
    "Bash(seite docs:*)",
    "Bash(seite docs)",
    "Bash(seite page:*)",
    "Bash(seite search:*)",
    "Bash(seite stats:*)",
    "Bash(seite stats)",
    "Bash(seite templates:*)",
    "Bash(seite templates)",
    "Bash(seite frontmatter:*)",
    "Bash(seite explain:*)",
    "Bash(seite hooks:*)",
    "Bash(seite collection list)",
];

/// Paths the agent may edit without asking (Claude `Write/Edit(...)`,
/// OpenCode `edit` rules).
const EDITABLE_PATHS: &[&str] = &["content/", "templates/", "static/", "data/"];
/// `seite` subcommands the agent may run without asking (OpenCode `bash`
/// rules; Claude's list is [`CLAUDE_ALLOWED_TOOLS`]).
const ALLOWED_COMMANDS: &[&str] = &[
    "seite build",
    "seite check",
    "seite new",
    "seite serve",
    "seite theme",
    "seite context",
    "seite docs",
    "seite page",
    "seite search",
    "seite stats",
    "seite templates",
    "seite frontmatter",
    "seite explain",
    "seite hooks",
    "seite collection list",
];

/// `.claude/settings.json` for a new site: permissions, plus (with `mcp`)
/// `enabledMcpjsonServers`, which pre-approves the `seite` server declared in
/// `.mcp.json`. (Claude Code ignores `mcpServers` in settings.json.)
pub fn claude_settings(mcp: bool) -> serde_json::Value {
    let mut allow: Vec<&str> = CLAUDE_ALLOWED_TOOLS.to_vec();
    if mcp {
        allow.push(CLAUDE_MCP_ALLOW);
    }
    let mut settings = serde_json::json!({
        "$schema": "https://json.schemastore.org/claude-code-settings.json",
        "permissions": {
            "allow": allow,
            "deny": ["Read(.env)", "Read(.env.*)"]
        }
    });
    if mcp {
        settings["enabledMcpjsonServers"] = serde_json::json!([MCP_SERVER]);
    }
    settings
}

fn claude_settings_template(mcp: bool) -> serde_json::Value {
    claude_settings(mcp)
}

/// OpenCode's `permission` block, mirroring the Claude allowlist: reads are
/// allowed (but `.env` files denied), edits to site sources and the listed
/// `seite` commands run without asking, anything else asks.
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

/// Allow rules for the Cursor CLI's project `.cursor/cli.json`: the `seite`
/// command and writes to site sources (plus [`CURSOR_MCP_ALLOW`] with
/// `--mcp`). Cursor matches `Shell(...)` on the command's first token, so
/// this allows every `seite` subcommand (Cursor still asks before anything
/// not listed here).
pub const CURSOR_ALLOW: &[&str] = &[
    "Shell(seite)",
    "Write(content/**)",
    "Write(templates/**)",
    "Write(static/**)",
    "Write(data/**)",
    "Write(seite.toml)",
];

/// Cursor's allow rule for the seite MCP server's tools.
pub const CURSOR_MCP_ALLOW: &str = "Mcp(seite:*)";

/// `.cursor/cli.json` for a new site (Cursor CLI permissions).
pub fn cursor_cli_json(mcp: bool) -> serde_json::Value {
    let mut allow: Vec<&str> = Vec::new();
    if mcp {
        allow.push(CURSOR_MCP_ALLOW);
    }
    allow.extend_from_slice(CURSOR_ALLOW);
    serde_json::json!({
        "permissions": {
            "allow": allow,
            "deny": ["Read(.env)", "Read(.env.*)"]
        }
    })
}

fn cursor_cli_json_template(mcp: bool) -> serde_json::Value {
    cursor_cli_json(mcp)
}

/// The `mcp.seite` entry in `opencode.json`.
pub fn opencode_mcp_entry() -> serde_json::Value {
    let mut command = vec![MCP_COMMAND];
    command.extend_from_slice(MCP_ARGS);
    serde_json::json!({ "type": "local", "command": command, "enabled": true })
}

/// `opencode.json` for a new site (with `mcp`: plus the `mcp.seite` server).
pub fn opencode_json(mcp: bool) -> serde_json::Value {
    let mut json = serde_json::json!({ "$schema": "https://opencode.ai/config.json" });
    if mcp {
        json["mcp"] = serde_json::json!({ MCP_SERVER: opencode_mcp_entry() });
    }
    json["permission"] = opencode_permission();
    json
}

fn opencode_json_template(mcp: bool) -> serde_json::Value {
    opencode_json(mcp)
}

/// `.claude/launch.json` entry that lets Claude's desktop app start the dev
/// server and show the site in its preview pane (live reload keeps it
/// current). https://code.claude.com/docs/en/desktop (preview servers)
pub const LAUNCH_JSON_PATH: &str = ".claude/launch.json";

/// The `seite` configuration in `.claude/launch.json`.
pub fn launch_configuration() -> serde_json::Value {
    serde_json::json!({
        "name": "seite",
        "runtimeExecutable": "seite",
        "runtimeArgs": ["serve", "--no-repl", "--port", "3000"],
        "port": 3000
    })
}

/// `.claude/launch.json` for a new site.
pub fn launch_json() -> serde_json::Value {
    serde_json::json!({
        "version": "0.0.1",
        "configurations": [launch_configuration()]
    })
}

/// Add the `seite` configuration to an existing `.claude/launch.json`,
/// keeping every other key and configuration. `Ok(true)` when added,
/// `Ok(false)` when a configuration named `seite` is already there, `Err`
/// when `configurations` isn't a list.
pub fn merge_launch_json(
    doc: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<bool, crate::cli::harness_hooks::Unmergeable> {
    if !doc.contains_key("version") {
        let rest = std::mem::take(doc);
        doc.insert("version".into(), serde_json::json!("0.0.1"));
        doc.extend(rest);
    }
    let list = doc
        .entry("configurations")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or(crate::cli::harness_hooks::Unmergeable)?;
    if list
        .iter()
        .any(|c| c.get("name").and_then(|n| n.as_str()) == Some("seite"))
    {
        return Ok(false);
    }
    list.push(launch_configuration());
    Ok(true)
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

/// The per-agent setup table for AGENTS.md (selected agents only): each
/// agent's project config and its one-time step. With `mcp`, an MCP column
/// and the MCP approval step are added.
pub fn mcp_setup_table(agents: &[Agent], mcp: bool) -> String {
    let mut md = if mcp {
        String::from("| Agent | Config | MCP config | One-time step |\n|---|---|---|---|\n")
    } else {
        String::from("| Agent | Config | One-time step |\n|---|---|---|\n")
    };
    for agent in agents {
        let spec = agent.spec();
        if mcp {
            let step = match (spec.setup_step, spec.mcp_step) {
                (step, "") => step.to_string(),
                ("None", mcp_step) => capitalize(mcp_step),
                (step, mcp_step) => format!("{step}; {mcp_step}"),
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                spec.label, spec.config_display, spec.mcp_config_display, step
            ));
        } else {
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                spec.label, spec.config_display, spec.setup_step
            ));
        }
    }
    md
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Whether any of `agents` already has the seite MCP server in its project
/// config (so AGENTS.md keeps documenting it without `--mcp`).
pub fn mcp_configured(root: &Path, agents: &[Agent]) -> bool {
    agents.iter().any(|agent| {
        let spec = agent.spec();
        let Ok(text) = std::fs::read_to_string(root.join(spec.mcp_config)) else {
            return false;
        };
        match spec.mcp_format {
            McpFormat::CodexToml => text
                .parse::<toml::Table>()
                .ok()
                .and_then(|t| t.get("mcp_servers")?.get(MCP_SERVER).cloned())
                .is_some(),
            McpFormat::McpServersJson => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .is_some_and(|v| v["mcpServers"].get(MCP_SERVER).is_some()),
            McpFormat::OpencodeJson => serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .is_some_and(|v| v["mcp"].get(MCP_SERVER).is_some()),
        }
    })
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
/// generated by `cli::init`). The MCP server configs are only planned with
/// `mcp` (`seite init --mcp`).
pub fn plan(agents: &[Agent], features: SiteFeatures, mcp: bool) -> Vec<PlannedFile> {
    let mut files = Vec::new();

    let mut skill_dirs: Vec<&str> = Vec::new();
    for spec in PROVIDERS.iter().filter(|p| has(agents, p.agent)) {
        if let Some(shim) = spec.instructions_shim {
            files.push(file(shim, crate::cli::agent_instructions::CLAUDE_SHIM));
        }
        let permissions_path = spec.permissions_file.map(|(path, _)| path);
        if let Some((path, permissions)) = spec.permissions_file {
            files.push(file(path, pretty_json(&permissions(mcp))));
        }
        // OpenCode's MCP server lives in its permissions file (rendered
        // above with `mcp`); the others get a separate MCP config.
        if mcp && permissions_path != Some(spec.mcp_config) {
            files.push(file(spec.mcp_config, spec.mcp_format.render()));
        }
        if spec.agent == Agent::Claude {
            files.push(file(LAUNCH_JSON_PATH, pretty_json(&launch_json())));
        }
        if let Some(native) = spec.rules {
            for r in rules(features) {
                files.push(file(
                    format!("{}/{}.{}", native.dir, r.name, native.ext),
                    native.format.render(r),
                ));
            }
        }
        // `.agents/skills` is shared by several agents; write it once.
        if !skill_dirs.contains(&spec.skills_dir) {
            skill_dirs.push(spec.skills_dir);
            for s in skills(features) {
                for (path, content) in skill_files(spec.skills_dir, s) {
                    files.push(file(path, content));
                }
            }
        }
    }
    for (path, content) in command_files(agents, features) {
        files.push(file(path, content));
    }
    if uses_shared_rules(agents) {
        for r in rules(features) {
            files.push(file(format!(".agents/rules/{}.md", r.name), claude_rule(r)));
        }
    }
    // Turn-end and post-edit `seite check` hooks (see `cli::harness_hooks`).
    crate::cli::harness_hooks::extend_plan(&mut files, agents);
    files
}

/// Write [`plan`] under `root`.
pub fn write_plan(
    root: &Path,
    agents: &[Agent],
    features: SiteFeatures,
    mcp: bool,
) -> std::io::Result<()> {
    for f in plan(agents, features, mcp) {
        let path = root.join(&f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, f.content)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Local docs (.seite/docs/)
// ---------------------------------------------------------------------------

/// Where `seite init` / `seite upgrade` write the embedded docs, so agents
/// read the docs that match the installed binary without a network call.
pub const LOCAL_DOCS_DIR: &str = ".seite/docs";

/// Every embedded how-to doc page as `(.seite/docs/<slug>.md, content)`.
/// The release notes (`releases`, ~60 KB of changelog) are left out: they
/// aren't reference material for working on a site.
pub fn local_docs() -> Vec<PlannedFile> {
    crate::docs::all()
        .into_iter()
        .filter(|d| d.slug != "releases")
        .map(|d| file(format!("{LOCAL_DOCS_DIR}/{}.md", d.slug), d.raw_content))
        .collect()
}

/// Write [`local_docs`] under `root`.
pub fn write_local_docs(root: &Path) -> std::io::Result<()> {
    for f in local_docs() {
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
    fn cursor_cli_json_allows_seite_and_denies_env() {
        let json = cursor_cli_json(false);
        let allow = json["permissions"]["allow"].as_array().unwrap();
        assert!(!allow.iter().any(|v| v == "Mcp(seite:*)"));
        assert!(allow.iter().any(|v| v == "Shell(seite)"));
        let deny = json["permissions"]["deny"].as_array().unwrap();
        assert!(deny.iter().any(|v| v == "Read(.env)"));
        let with_mcp = cursor_cli_json(true);
        assert_eq!(with_mcp["permissions"]["allow"][0], "Mcp(seite:*)");
    }

    #[test]
    fn claude_settings_only_wire_mcp_with_flag() {
        let plain = claude_settings(false);
        assert!(plain.get("enabledMcpjsonServers").is_none());
        let allow = plain["permissions"]["allow"].as_array().unwrap();
        assert!(!allow.iter().any(|v| v == "mcp__seite"));
        // The agent-facing commands run without a prompt; deploy still asks.
        for rule in [
            "Bash(seite context)",
            "Bash(seite docs:*)",
            "Bash(seite hooks:*)",
        ] {
            assert!(allow.iter().any(|v| v == rule), "{rule}");
        }
        assert!(!allow.iter().any(|v| v.as_str().unwrap().contains("deploy")));
        assert!(!allow.iter().any(|v| v == "Bash(seite:*)"));
        let mcp = claude_settings(true);
        assert_eq!(mcp["enabledMcpjsonServers"][0], "seite");
        assert!(mcp["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "mcp__seite"));
        // Every rule `seite upgrade` adds is also in a new site's settings.
        for rule in CLAUDE_ALLOWED_TOOLS_UPGRADE {
            assert!(CLAUDE_ALLOWED_TOOLS.contains(rule), "{rule}");
        }
    }

    #[test]
    fn opencode_json_shape() {
        let json = opencode_json(true);
        assert_eq!(json["$schema"], "https://opencode.ai/config.json");
        assert_eq!(json["mcp"]["seite"]["type"], "local");
        assert_eq!(
            json["mcp"]["seite"]["command"],
            serde_json::json!(["seite", "mcp"])
        );
        assert_eq!(json["mcp"]["seite"]["enabled"], true);
        assert!(opencode_json(false).get("mcp").is_none());
        let perm = &json["permission"];
        assert_eq!(perm["bash"]["*"], "ask");
        assert_eq!(perm["bash"]["seite build*"], "allow");
        assert_eq!(perm["bash"]["seite theme*"], "allow");
        assert_eq!(perm["bash"]["seite context*"], "allow");
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
    fn merge_codex_config_without_servers_inline_seite_and_odd_shapes() {
        // No mcp_servers at all: the table is appended after the user's text.
        let existing = "model = \"o3\" # pinned\n";
        let merged = merge_codex_config(existing).unwrap().unwrap();
        assert!(
            merged.starts_with("model = \"o3\" # pinned\n\n"),
            "{merged}"
        );
        let parsed: toml::Value = toml::from_str(&merged).unwrap();
        assert_eq!(parsed["model"].as_str(), Some("o3"));
        assert_eq!(
            parsed["mcp_servers"]["seite"]["command"].as_str(),
            Some("seite")
        );
        // Already declared inline: nothing to do.
        let inline = "mcp_servers = { seite = { command = \"seite\" } }\n";
        assert!(merge_codex_config(inline).unwrap().is_none());
        // `mcp_servers` that isn't a table can't be extended: leave it alone
        // rather than replacing the user's value.
        assert!(merge_codex_config("mcp_servers = \"oops\"\n")
            .unwrap()
            .is_none());
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
        let table = mcp_setup_table(&[Agent::Codex, Agent::Opencode], false);
        assert!(table.starts_with("| Agent | Config | One-time step |"));
        assert!(table.contains("`.codex/hooks.json`") && table.contains("`opencode.json`"));
        assert!(!table.contains(".mcp.json") && !table.contains("Cursor"));
        assert!(!table.contains("MCP"), "{table}");

        let table = mcp_setup_table(&[Agent::Claude, Agent::Opencode], true);
        assert!(table.starts_with("| Agent | Config | MCP config | One-time step |"));
        assert!(table.contains("`.mcp.json`") && table.contains("approve the server with `/mcp`"));
        // OpenCode has no setup step of its own and no MCP step.
        assert!(table.contains("| OpenCode | `opencode.json`, `.opencode/plugins/seite-check.js` | `opencode.json` | None |"), "{table}");
    }

    #[test]
    fn mcp_configured_detects_each_format() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        assert!(!mcp_configured(root, &Agent::ALL));
        std::fs::write(
            root.join("opencode.json"),
            pretty_json(&opencode_json(false)),
        )
        .unwrap();
        assert!(!mcp_configured(root, &Agent::ALL));
        std::fs::write(
            root.join("opencode.json"),
            pretty_json(&opencode_json(true)),
        )
        .unwrap();
        assert!(mcp_configured(root, &[Agent::Opencode]));
        assert!(!mcp_configured(root, &[Agent::Claude]));
        std::fs::create_dir_all(root.join(".codex")).unwrap();
        std::fs::write(root.join(".codex/config.toml"), codex_config_toml()).unwrap();
        assert!(mcp_configured(root, &[Agent::Codex]));
        std::fs::write(root.join(".mcp.json"), pretty_json(&mcp_json())).unwrap();
        assert!(mcp_configured(root, &[Agent::Claude]));
    }

    #[test]
    fn plan_without_mcp_writes_no_mcp_config() {
        for agent in Agent::ALL {
            let files = plan(&[agent], ALL_FEATURES, false);
            for f in &files {
                assert!(
                    !f.content.contains("mcpServers")
                        && !f.content.contains("mcp_servers")
                        && !f.content.contains("mcp__seite")
                        && !f.content.contains("Mcp(seite")
                        && !f.content.contains("enabledMcpjsonServers"),
                    "{agent:?}: {}",
                    f.path
                );
            }
            let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
            assert!(!paths.contains(&".mcp.json") && !paths.contains(&".codex/config.toml"));
            assert!(!paths.contains(&".cursor/mcp.json"));
        }
        // With --mcp each agent gets its MCP config.
        let files = plan(&Agent::ALL, ALL_FEATURES, true);
        for spec in &PROVIDERS {
            let f = files.iter().find(|f| f.path == spec.mcp_config).unwrap();
            assert!(f.content.contains("seite"), "{}", spec.mcp_config);
        }
        let gemini = files
            .iter()
            .find(|f| f.path == ".gemini/settings.json")
            .unwrap();
        let gemini: serde_json::Value = serde_json::from_str(&gemini.content).unwrap();
        assert_eq!(gemini["mcpServers"]["seite"]["command"], "seite");
        assert!(gemini["hooks"].get("AfterAgent").is_some());
    }

    #[test]
    fn plan_subset_writes_only_selected_agent_files() {
        let paths: Vec<String> = plan(&[Agent::Claude], ALL_FEATURES, false)
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert!(paths.contains(&".claude/settings.json".to_string()));
        assert!(paths.contains(&LAUNCH_JSON_PATH.to_string()));
        assert!(paths.contains(&".claude/skills/seite/reference/theme.md".to_string()));
        assert!(paths.iter().all(|p| !p.starts_with(".cursor")
            && !p.starts_with(".codex")
            && !p.starts_with(".agents")
            && !p.starts_with(".gemini")
            && p != "opencode.json"
            && p != "GEMINI.md"));

        let paths: Vec<String> = plan(&[Agent::Codex], SiteFeatures::default(), false)
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert!(paths.contains(&".codex/hooks.json".to_string()));
        assert!(paths.contains(&".agents/skills/seite/SKILL.md".to_string()));
        assert!(paths.contains(&".agents/rules/templates.md".to_string()));
        assert!(paths
            .iter()
            .all(|p| !p.starts_with(".claude") && p != "CLAUDE.md"));
        assert!(!paths.iter().any(|p| p.contains("design-prompts")
            || p.contains("theme-builder")
            || p.contains("brand-identity")
            || p.contains("landing-page")));

        let paths: Vec<String> = plan(&[Agent::Gemini], SiteFeatures::default(), false)
            .into_iter()
            .map(|f| f.path)
            .collect();
        for p in [
            "GEMINI.md",
            ".gemini/settings.json",
            ".gemini/commands/seite.toml",
            ".agents/skills/seite/SKILL.md",
            ".agents/rules/templates.md",
        ] {
            assert!(paths.contains(&p.to_string()), "{p}: {paths:?}");
        }
    }

    #[test]
    fn provider_table_matches_agent_order() {
        for (i, agent) in Agent::ALL.into_iter().enumerate() {
            assert_eq!(PROVIDERS[i].agent, agent);
            assert_eq!(agent.spec().agent, agent);
        }
        let ids: Vec<_> = Agent::ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids, ["claude", "codex", "opencode", "cursor", "gemini"]);
        assert_eq!(Agent::from_id("gemini"), Some(Agent::Gemini));
    }

    #[test]
    fn launch_json_merge_is_additive() {
        let mut doc: serde_json::Map<String, serde_json::Value> = serde_json::from_value(
            serde_json::json!({ "configurations": [{ "name": "api", "port": 8080 }] }),
        )
        .unwrap();
        assert_eq!(merge_launch_json(&mut doc), Ok(true));
        assert_eq!(merge_launch_json(&mut doc), Ok(false));
        assert_eq!(doc.keys().next().unwrap(), "version");
        let configs = doc["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0]["name"], "api");
        assert_eq!(configs[1], launch_configuration());
        assert_eq!(
            configs[1]["runtimeArgs"],
            serde_json::json!(["serve", "--no-repl", "--port", "3000"])
        );
        let mut bad: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({ "configurations": {} })).unwrap();
        assert!(merge_launch_json(&mut bad).is_err());
        assert_eq!(launch_json()["version"], "0.0.1");
    }

    /// Split a SKILL.md into (frontmatter YAML, body).
    fn split_frontmatter(content: &str) -> (serde_yaml_ng::Mapping, &str) {
        let (fm, body) = content
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .expect("frontmatter");
        (serde_yaml_ng::from_str(fm).unwrap(), body)
    }

    fn keys(map: &serde_yaml_ng::Mapping) -> Vec<&str> {
        map.keys().filter_map(|k| k.as_str()).collect()
    }

    #[test]
    fn every_skill_is_portable_in_the_shared_dir() {
        // Codex rejects unknown top-level keys; the version is a YAML comment.
        for skill in SKILLS {
            let (fm, _) = split_frontmatter(&skill.render(SkillFrontmatter::Portable));
            assert_eq!(keys(&fm), ["name", "description"], "{}", skill.name);
            assert_eq!(fm["name"].as_str(), Some(skill.name));
            assert_eq!(fm["description"].as_str(), Some(skill.description()));
            assert!(skill.version() >= 1, "{}", skill.name);
        }
    }

    #[test]
    fn seite_skill_gets_argument_hint_only_for_claude() {
        let seite = skill("seite").unwrap();
        let rendered = seite.render(SkillFrontmatter::ClaudeCode);
        let (fm, body) = split_frontmatter(&rendered);
        assert_eq!(keys(&fm), ["name", "description", "argument-hint"]);
        assert!(fm["argument-hint"]
            .as_str()
            .unwrap()
            .starts_with("[check | new"));
        assert_eq!(extract_version(&rendered), 3);
        assert!(body.contains("## Routing"));
        assert_eq!(SKILLS.len(), 1, "one routed skill");
    }

    #[test]
    fn seite_skill_routes_every_command_to_a_reference() {
        let seite = skill("seite").unwrap();
        assert!(
            seite.content.len() < 4 * 1024,
            "seite skill is {} bytes",
            seite.content.len()
        );
        assert!(seite.content.contains("`/seite <command> [args]`"));
        assert!(seite.content.contains("seite context"));
        // Bounded verification, no shouting.
        assert!(seite.content.contains("then stop"));
        assert!(!seite.content.contains("MANDATORY"));
        let names: Vec<_> = seite.references.iter().map(|r| r.name).collect();
        assert_eq!(
            names,
            [
                "check",
                "new",
                "preview",
                "build",
                "deploy",
                "theme",
                "brand",
                "landing",
                "collection",
                "upgrade",
                "hooks"
            ]
        );
        for r in seite.references {
            let link = format!("[reference/{0}.md](reference/{0}.md)", r.name);
            assert!(seite.content.contains(&link), "{link}");
            assert!(
                r.content.starts_with(&format!("# /seite {}", r.name)),
                "{}",
                r.name
            );
            assert!(!r.content.contains("MANDATORY"), "{}", r.name);
            // References are plain markdown (no frontmatter) and stay short.
            assert!(r.content.len() < 4 * 1024, "{}", r.name);
            // No MCP tool names: coding agents use the CLI.
            assert!(!r.content.contains("seite_"), "{}", r.name);
        }
        let theme = seite.references.iter().find(|r| r.name == "theme").unwrap();
        for needle in [
            "/impeccable",
            "impeccable detect dist/",
            "never make standalone HTML mockups",
            "search-index.json",
            "{{ t.",
            "`content` (inside",
        ] {
            assert!(
                theme
                    .content
                    .to_lowercase()
                    .contains(&needle.to_lowercase()),
                "{needle}"
            );
        }
        let preview = seite
            .references
            .iter()
            .find(|r| r.name == "preview")
            .unwrap();
        assert!(preview.content.contains(".claude/launch.json"));
        assert!(preview.content.contains("Never start a second server"));
        let brand = seite.references.iter().find(|r| r.name == "brand").unwrap();
        assert!(brand.content.contains("data/brand.yaml"));
        let landing = seite
            .references
            .iter()
            .find(|r| r.name == "landing")
            .unwrap();
        assert!(landing.content.contains("content/pages/index.md"));
    }

    #[test]
    fn command_wrappers_route_to_the_seite_skill() {
        let files = command_files(&Agent::ALL, SiteFeatures::default());
        let paths: Vec<_> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [".opencode/commands/seite.md", ".gemini/commands/seite.toml"]
        );
        let (_, content) = &files[0];
        let (fm, body) = split_frontmatter(content);
        assert_eq!(keys(&fm), ["description"]);
        assert_eq!(
            fm["description"].as_str(),
            Some(skill("seite").unwrap().description())
        );
        assert!(body.contains("skill({ name: \"seite\" })"), "{body}");
        assert!(body.contains("$ARGUMENTS"));
        assert_eq!(extract_version(content), 3);

        let (_, gemini) = &files[1];
        let parsed: toml::Table = toml::from_str(gemini).unwrap();
        assert_eq!(
            parsed["description"].as_str(),
            Some(skill("seite").unwrap().description())
        );
        assert!(parsed["prompt"].as_str().unwrap().contains("{{args}}"));
        assert_eq!(extract_version(gemini), 3);
        assert!(
            command_files(&[Agent::Claude, Agent::Codex, Agent::Cursor], ALL_FEATURES).is_empty()
        );
    }

    #[test]
    fn plan_writes_seite_skill_and_references_for_every_agent() {
        for agent in Agent::ALL {
            let files = plan(&[agent], SiteFeatures::default(), false);
            let dir = agent.spec().skills_dir;
            let skill = files
                .iter()
                .find(|f| f.path == format!("{dir}/seite/SKILL.md"))
                .unwrap_or_else(|| panic!("{agent:?}"));
            assert_eq!(
                skill.content.contains("argument-hint:"),
                agent == Agent::Claude
            );
            for r in SEITE_REFERENCES {
                let path = format!("{dir}/seite/reference/{}.md", r.name);
                let f = files
                    .iter()
                    .find(|f| f.path == path)
                    .unwrap_or_else(|| panic!("{agent:?}: {path}"));
                assert_eq!(f.content, r.content);
            }
            assert_eq!(
                files
                    .iter()
                    .any(|f| f.path == ".opencode/commands/seite.md"),
                agent == Agent::Opencode
            );
        }
        // No path is planned twice.
        let files = plan(&Agent::ALL, ALL_FEATURES, true);
        let mut paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        let n = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), n);
    }

    #[test]
    fn local_docs_cover_every_embedded_page() {
        let docs = local_docs();
        assert_eq!(docs.len(), crate::docs::all().len() - 1);
        assert!(!docs.iter().any(|f| f.path.ends_with("releases.md")));
        assert!(docs
            .iter()
            .any(|f| f.path == ".seite/docs/getting-started.md"));
        let tmp = tempfile::TempDir::new().unwrap();
        write_local_docs(tmp.path()).unwrap();
        assert!(tmp.path().join(".seite/docs/agent.md").is_file());
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn detect_installed_agents_from_path_and_home() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bin_a = tmp.path().join("bin-a");
        let bin_b = tmp.path().join("bin-b");
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let path = std::env::join_paths([&bin_a, &bin_b]).unwrap();

        assert!(detect_installed_agents(Some(&path), Some(&home)).is_empty());
        assert!(detect_installed_agents(None, None).is_empty());

        touch(&bin_b.join("codex"));
        touch(&bin_a.join("cursor-agent"));
        assert_eq!(
            detect_installed_agents(Some(&path), Some(&home)),
            vec![Agent::Codex, Agent::Cursor]
        );

        // Config dirs count too; a plain file named like one does not.
        std::fs::create_dir_all(home.join(".config/opencode")).unwrap();
        touch(&home.join(".claude"));
        assert_eq!(
            detect_installed_agents(Some(&path), Some(&home)),
            vec![Agent::Codex, Agent::Opencode, Agent::Cursor]
        );
        std::fs::remove_file(home.join(".claude")).unwrap();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        assert_eq!(
            detect_installed_agents(None, Some(&home)),
            vec![Agent::Claude, Agent::Opencode]
        );
        // A directory named like a binary is not a binary.
        let other = tmp.path().join("bin-c");
        std::fs::create_dir_all(other.join("claude")).unwrap();
        let other_path = std::env::join_paths([&other]).unwrap();
        assert!(detect_installed_agents(Some(&other_path), None).is_empty());
    }

    #[test]
    fn interactive_defaults_fall_back_to_all() {
        assert_eq!(interactive_defaults(&[]), vec![true; 5]);
        assert_eq!(
            interactive_defaults(&[Agent::Claude, Agent::Cursor]),
            vec![true, false, false, true, false]
        );
    }
}
