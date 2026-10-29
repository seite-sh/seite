---
title: "Coding Agents"
description: "Run your seite site with Claude Code, Codex, OpenCode, Cursor, Antigravity, Gemini CLI or Copilot: one AGENTS.md, one /seite skill, check hooks, no API keys."
weight: 10
---

seite is built so a coding agent can run a site end to end: write content, design the theme, check its own work, preview, and deploy. Seven agents are supported: Claude Code, Codex CLI, OpenCode, Cursor, Gemini CLI, GitHub Copilot, and Antigravity CLI. seite itself needs no API keys or paid services; OpenCode can also run free and local models. The agent gets its context from the project itself, and the `seite` CLI does the work that has to be right every time.

## What `seite init` sets up

`seite init --agents claude,codex,opencode,cursor,gemini,copilot,antigravity` (default: all; the interactive picker preselects the agents found on your machine) writes:

- **`AGENTS.md`**: a compact project guide every agent reads. It covers what the site is, the layout, the collections, the content format, the hard rules, and the workflow. Claude Code and Gemini CLI read it through one-line `CLAUDE.md` / `GEMINI.md` files that import it; GitHub Copilot and Antigravity CLI read `AGENTS.md` directly.
- **The `seite` skill**: one routed skill, `/seite <command>` (Codex: `$seite <command>`), with a short `SKILL.md` and one reference file per command in `reference/`. Claude Code reads it from `.claude/skills/seite/`; Codex, Cursor, OpenCode, Gemini CLI, GitHub Copilot, and Antigravity CLI read `.agents/skills/seite/`. OpenCode gets a `/seite` command wrapper in `.opencode/commands/`, Gemini CLI in `.gemini/commands/seite.toml`.
- **Context rules**: path-scoped guides (templates, SEO, i18n, shortcodes, data files, config) in `.claude/rules/`, `.cursor/rules/`, and `.github/instructions/*.instructions.md` (Copilot, `applyTo` globs), and `.agents/rules/` (Antigravity CLI, `trigger: glob`), loaded automatically for matching files. Codex, OpenCode, and Gemini CLI find them through the index in `AGENTS.md` (`.agents/rules/` when no Claude, Cursor, or Copilot rules exist).
- **Local docs**: `.seite/docs/` holds the documentation for the installed seite version, so agents look things up without a network call. `seite docs <query>` searches it. It is gitignored (not committed): `seite context`, `seite docs`, and `seite upgrade` write it when it is missing or from another seite version, and drop pages the version no longer ships.
- **Permissions**: `.claude/settings.json`, `.cursor/cli.json`, and `opencode.json` let the agent read the site, edit `content/`, `templates/`, `static/`, `data/`, and `seite.toml`, and run these `seite` commands without asking: `build`, `check`, `new`, `serve`, `audit`, `preview`, `og preview`, `context`, `docs`, `page`, `search`, `stats`, `templates`, `frontmatter`, `explain`, `hooks`, and `collection list`. Among the theme commands, only `seite theme apply` and `seite theme list` are allowed; `seite theme create`, `seite agent`, `seite deploy`, and everything else still ask. `.env` files are never readable. `seite upgrade` narrows the broader rules seite 0.20 wrote (`Bash(seite theme:*)`, `Shell(seite)`, `"seite theme*"`) when they are still exactly as written, and adds the newer agent commands (including `audit`, `preview` and `og preview` for Claude Code).
- **Check hooks**: see [Check hooks](#check-hooks).
- **Claude desktop preview**: `.claude/launch.json` declares the `seite` dev server, so the Claude desktop app opens the site in its preview pane with live reload.

The MCP server is not wired in by default: coding agents use the CLI. Pass `--mcp` to add it as well (see [MCP Server](/docs/mcp-server)).

Each agent has a one-time step before the project config takes effect. `AGENTS.md` lists them under "Agent setup":

| Agent | One-time step |
|---|---|
| Claude Code | Accept the workspace trust prompt |
| Codex CLI | Trust the project, then approve the hooks in `/hooks` |
| OpenCode | None |
| Cursor | Trust the workspace (`cursor-agent --trust` or the prompt) |
| Gemini CLI | Trust the folder (`gemini trust`) and approve the project hooks |
| GitHub Copilot | Trust the folder when Copilot CLI asks (repository hooks load only in trusted folders) |
| Antigravity CLI | None; for `agy -p`, allow seite commands in your user settings (see [Antigravity CLI](#antigravity-cli)) |

## The `/seite` skill

| Command | What the agent does |
|---|---|
| `check` | `seite check`, fixes every error at its file and line, checks once more, then stops |
| `audit` | `seite audit --json`, fixes SEO, AI-visibility and accessibility findings at the file and line each names, then audits once more |
| `new <type> "<title>"` | Creates the file with `seite new`, writes it, checks it |
| `preview` | Opens the running dev server, or starts one (built-in browser in Claude's desktop app and Cursor, else `seite serve --open`) |
| `build` | `seite build --json`, reports broken links, missing assets, diagnostics |
| `deploy` | Checks, runs `seite deploy --dry-run`, asks you before the real deploy |
| `theme [name \| description]` | Applies a bundled theme, or designs a custom one in `templates/base.html` |
| `brand` | Logo, palette (`data/brand.yaml`), favicon |
| `landing [page]` | Homepage or landing page: content in `content/pages/`, template in `templates/` |
| `collection [preset]` | Lists collections or adds a preset |
| `upgrade` | `seite upgrade --check`, then applies it after you agree |
| `hooks [on \| off \| status]` | Manages the check hooks |

With no command, the agent picks one from your request and asks once if two fit. Each session starts with `seite context`, which prints the config, collections, theme, dev server, and pending upgrades in one go.

### Design with impeccable

For design work, install the optional [impeccable](https://github.com/pbakaus/impeccable) skill: `seite skill install impeccable`. When it is present, `/seite theme` uses `/impeccable` for the design decisions and keeps seite's template contract (required blocks, SEO head, i18n, search). It builds directly in `templates/`, previews with `seite serve`, and finishes with `seite check` plus one `impeccable detect dist/`.

## Check hooks

Each agent gets two hooks that run `seite check` for it and hand errors back. Warnings never block.

- **After each edit**: `seite check --hook <agent> --post-edit` checks only the edited file (frontmatter, shortcodes, template syntax, data-file syntax) and returns its errors with the tool result.
- **At the end of a turn**: `seite check --hook <agent>` checks the whole site. With errors, the agent keeps working; it is sent back at most once per turn.

| Agent | Hook config | After edit | End of turn |
|---|---|---|---|
| Claude Code | `.claude/settings.json` | `PostToolUse` (`Write\|Edit\|MultiEdit`) | `Stop` |
| Codex CLI | `.codex/hooks.json` | `PostToolUse` (`apply_patch`) | `Stop` |
| Cursor | `.cursor/hooks.json` | `postToolUse` (`Write`) | `stop` |
| OpenCode | `.opencode/plugins/seite-check.js` | `tool.execute.after` | `session.idle` |
| Gemini CLI | `.gemini/settings.json` | `AfterTool` (`write_file\|replace`) | `AfterAgent` |
| GitHub Copilot | `.github/hooks/seite.json` | `PostToolUse` (`edit\|create\|Edit\|Write\|MultiEdit`) | `Stop` |
| Antigravity CLI | `.agents/hooks.json` | `PostToolUse` (file tools) + `PostInvocation` | `Stop` |

Every hook command is wrapped so a missing `seite` binary exits 0 silently (`command -v seite >/dev/null 2>&1 && seite check --hook … || true`); a collaborator without seite is never blocked. The wrapper is POSIX shell syntax: on Windows, hooks need Git Bash (Cursor runs hooks in PowerShell there, where the wrapper does not work; Copilot's hooks set only the `bash` command, so they don't run under PowerShell). The hook exits 0 on every path and is silent outside a seite site.

```bash
seite hooks status                 # installed / missing, per agent
seite hooks off --agents cursor    # remove seite's hook entries only
seite hooks off --tier post-edit   # keep the turn-end check, drop the after-edit one
seite hooks on                     # install them again
```

`.seite/config.json` records each hook tier seite installed (`hooks_installed`: `claude` for the turn-end hook, `claude:post-edit` for the after-edit hook). `seite upgrade` adds a tier only when it isn't recorded, so a hook you turned off or deleted stays off, tier by tier. Sites from seite 0.20 recorded only the turn-end tier: upgrade adds their after-edit hook while the turn-end hook is still in place. `seite hooks off` deletes OpenCode's plugin only while it is exactly as seite wrote it, and removes only hook entries whose command is one seite generated.

## `seite agent`

`seite agent` opens a coding agent in the site. It adds no context, tool allowlist, or MCP flags: the prompt is passed through unchanged, the agent reads `AGENTS.md` and the `/seite` skill, and it runs with the project's own permissions. It must be run inside a site (a directory with `seite.toml`):

```bash
seite agent                                     # interactive session
seite agent "write a post about our launch"     # interactive, starting with this prompt
seite agent --once "fix all check errors"       # run once and exit
seite agent --with gemini "add a docs page"     # another agent
```

| `--with` | Interactive with a prompt | One-shot (`--once`) | Install |
|---|---|---|---|
| `claude` (default) | `claude "<prompt>"` | `claude -p "<prompt>"` | `npm install -g @anthropic-ai/claude-code` |
| `codex` | `codex "<prompt>"` | `codex exec --sandbox workspace-write "<prompt>"` | `npm install -g @openai/codex` |
| `opencode` | `opencode --prompt "<prompt>"` | `opencode run "<prompt>"` | `curl -fsSL https://opencode.ai/install \| bash` |
| `cursor` | `cursor-agent "<prompt>"` | `cursor-agent -p "<prompt>" --output-format text` | `curl https://cursor.com/install -fsS \| bash` |
| `gemini` | `gemini -i "<prompt>"` | `gemini -p "<prompt>"` | `npm install -g @google/gemini-cli` |
| `copilot` | `copilot -i "<prompt>"` | `copilot -p "<prompt>"` | `npm install -g @github/copilot` |
| `antigravity` | `agy -i "<prompt>"` | `agy -p "<prompt>"` | `curl -fsSL https://antigravity.google/cli/install.sh \| bash` |

`SEITE_AGENT` sets the default harness. If the binary isn't on `PATH`, `seite agent` fails right away with the install command.

Auto-approval is a separate, explicit flag: `seite agent --once --auto-approve "<prompt>"` adds `opencode run --auto`, `cursor-agent --force --approve-mcps`, `gemini --approval-mode=yolo`, `copilot --allow-all-tools`, or `agy --dangerously-skip-permissions`. Claude Code and Codex get no extra flag. It only takes effect with `--once`, from an interactive terminal, outside any agent session; otherwise seite prints a warning and runs without it. The global `--yes` (or `SEITE_YES=1`) never adds it.

`seite agent` and `seite theme create` refuse to start a nested agent when they run inside a coding agent's session (detected from the variables the agents set for their commands: `CLAUDECODE`, `CODEX_THREAD_ID`, `CODEX_SANDBOX`, `GEMINI_CLI`, `OPENCODE`, `CURSOR_AGENT`, `ANTIGRAVITY_AGENT`, `ANTIGRAVITY_CONVERSATION_ID`, and the hook project-dir variables; Copilot documents no such variable, so a session inside Copilot is not detected). The error tells you to run `/seite theme …` (or the request) directly in the session you are in.

## `seite theme create`

```bash
seite theme create "dark mode with neon green accents"
```

With impeccable installed, this prints how to run `/impeccable` with your description. Otherwise it runs your agent once with `/seite theme <description>` (`$seite theme …` in Codex), then suggests `seite skill install impeccable` for deeper design work. It never passes an auto-approve flag, so the agent asks before anything its permissions don't allow.

With `SEITE_AGENT=copilot`, the prompt is "Use the /seite skill: theme <description>", since Copilot runs a skill that the prompt names.

With `SEITE_AGENT=gemini`, Gemini CLI may not expand custom commands such as `/seite` in headless (`-p`) mode (unverified); if the theme isn't generated, run `/seite theme <description>` in an interactive `gemini` session instead.

## GitHub Copilot

`seite init --agents copilot` (or `seite upgrade --agents …,copilot`) sets a site up for Copilot CLI, the Copilot cloud agent, and agent mode in VS Code:

- Copilot reads `AGENTS.md` itself, so no extra instructions file is written.
- The skill goes to `.agents/skills/seite/`, which Copilot loads. Ask for it by name: "Use the /seite skill to check the site".
- Context rules go to `.github/instructions/<name>.instructions.md` with `applyTo` globs.
- Hooks go to `.github/hooks/seite.json` (`Stop` and `PostToolUse`), the same two-tier check as the other agents. In the cloud agent, where `seite` is usually not installed, they exit silently.
- With `--mcp`, Copilot uses the same `.mcp.json` as Claude Code. Copilot gives `.mcp.json` precedence over `.github/mcp.json`, so seite writes only `.mcp.json`.
- Copilot has no project permissions file, so seite writes none. Approve tools when Copilot asks, or pass `--allow-tool` yourself.

## Antigravity CLI

`seite init --agents antigravity` (or `seite upgrade --agents …,antigravity`) sets a site up for Antigravity CLI (`agy`), Google's successor to Gemini CLI. Gemini CLI stays supported separately.

- Antigravity reads `AGENTS.md` itself, so no extra instructions file is written.
- The skill goes to `.agents/skills/seite/`; Antigravity turns it into the `/seite` slash command.
- Context rules go to `.agents/rules/<name>.md` with `trigger: glob` frontmatter (`globs` from each rule's paths), so they load when the agent works on matching files. Antigravity discards a rule file without a valid `trigger`, so every `.agents/rules/` copy carries this frontmatter, also on sites where it only serves Codex, OpenCode, or Gemini CLI; `seite upgrade` rewrites copies an earlier seite wrote with Claude's `paths:` frontmatter (edited copies are kept, with a notice).
- Hooks go to `.agents/hooks.json` under a hook named `seite-check`. The turn-end check is a `Stop` hook: with errors it answers `{"decision": "continue"}` and the errors become a system message, once per turn. Antigravity's `PostToolUse` hook has no way to talk back to the model, so the after-edit check takes two hooks: `PostToolUse` on the file tools (`write_file`, `edit_file`, and the IDE's `write_to_file`/`replace_file_content`/`multi_replace_file_content`) records the edited file in seite's per-user cache folder (outside the project), and `PostInvocation` checks the recorded files once the tool calls have run and hands errors back as a message before the next model call. Workspace hooks load without a trust step.
- With `--mcp`, the server goes to `.agents/mcp_config.json` (`mcpServers.seite`).
- Antigravity has no project permissions file: permissions live only in your `~/.gemini/antigravity-cli/settings.json`. Interactive sessions ask before running a command. Headless `agy -p` (`seite agent --with antigravity --once`) cannot ask and denies any shell command no allow rule covers, so add rules for the seite commands it should run, for example `"permissions": {"allow": ["command(seite check)", "command(seite context)"]}`. A rule matches that exact command line only (in our tests with agy 1.4.10, `command(seite)` and `command(seite *)` did not cover `seite --version`), so headless runs work best for tasks whose commands you can list.
- `agy` runs shell commands in a login shell: a `seite` on the `PATH` of the terminal you started it from but not in your shell profile is not found by the agent.

## Using seite with an existing site

`seite init` is not required. For a site you already have, or one made with an older seite, two installs give your agent the `/seite` skill without touching the project files:

**Any agent, with the [skills CLI](https://skills.sh)** ([vercel-labs/skills](https://github.com/vercel-labs/skills)):

```bash
npx skills add seite-sh/seite              # pick agents interactively
npx skills add seite-sh/seite -a claude-code -a codex   # or name them
npx skills add seite-sh/seite -g           # install for your user, every project
```

This copies the `seite` skill (`SKILL.md` and its `reference/` files) from the `skills/seite/` directory of the seite repository into each agent's skills directory (`.claude/skills/` for Claude Code; `.agents/skills/` for Codex, Cursor, OpenCode, Gemini CLI, GitHub Copilot, and Antigravity CLI). It installs only the skill: no hooks, no rules, no permissions.

**Claude Code, as a plugin** (the skill plus the check hooks):

```text
/plugin marketplace add seite-sh/seite
/plugin install seite@seite
```

From a shell, `claude plugin marketplace add seite-sh/seite` and `claude plugin install seite@seite`. The plugin carries the `/seite` skill and the same two hooks as `.claude/settings.json` (after each edit, and at the end of a turn), run as `seite check --hook claude --plugin`. It adds no MCP server.

A plugin's hooks fire in every project where it is enabled, including repositories you only cloned, so the plugin's copy is scoped tightly:

- Outside a seite site it costs almost nothing. `command -v seite` is a shell builtin, and `seite check --hook claude --plugin` looks for a `seite.toml` in the working directory and its parents. Without one, it exits 0 and prints nothing, and it skips the update check and telemetry.
- The end-of-turn check runs only in a turn where the agent edited a file inside the site. The after-edit hook leaves a marker in `.seite/cache/`, and the end-of-turn hook runs only when it finds the marker, which it removes.
- When the project's `.claude/settings.json` or `.claude/settings.local.json` already runs seite's hooks (from `seite init` or `seite hooks on`), the plugin's copy steps aside, so each check runs once.

Every hook message starts with a line saying it is `seite check` output about the site's files, and each diagnostic line is cut at 300 characters, since diagnostics quote file content. `seite hooks` manages only the project's hooks; turn the plugin's hooks off by disabling the plugin in `/plugin`.

Both installs need the `seite` CLI on your `PATH` (`curl -fsSL https://seite.sh/install.sh | sh`). The skill checks for it and asks before installing it. They come from the repository's default branch, so they can be newer than your installed `seite`; `seite self-update` brings the binary up to date. `seite upgrade --agents …` later adds the full set (rules, permissions, project hooks) for a project.

## Keeping a site current

```bash
seite upgrade --check     # list pending changes (exit 1 when there are some)
seite upgrade             # apply them
seite upgrade --agents claude,gemini   # change the agent selection
seite upgrade --mcp       # also wire in the MCP server
seite upgrade --agent     # upgrade, then hand the release notes to an agent as a migration brief
seite upgrade --agent --with codex   # the agent to start (default: SEITE_AGENT, else claude)
```

`seite upgrade --agent` collects the release notes since the project's recorded seite version into a brief and saves it to `.seite/upgrade-brief.md` (gitignored). From a terminal it then starts the agent interactively with "Read .seite/upgrade-brief.md and follow it", never with an auto-approve flag. Inside an agent session, or with `--json` (`data.brief`), it prints the brief instead, so the agent you are talking to applies it. It cannot be combined with `--check`.

Upgrade never adds an agent on its own. Sites from seite 0.20 that store their selection keep it, so Gemini CLI, GitHub Copilot, and Antigravity CLI are added only when `--agents` names them (`seite upgrade --agents <current>,gemini,copilot,antigravity`); upgrade says once that they are available. A project with no stored selection (from before seite 0.20, or added with 0.20's `seite workspace add`) is maintained for Claude Code, Codex, OpenCode, Cursor, and Gemini CLI. Upgrade adds missing files for the selected agents and merges into existing configs without replacing your settings. It replaces the `seite` skill and its references when the bundled version is newer, refreshes `.seite/docs/` after a seite update, and adds `/.seite/docs/` and `/.seite/dev.json` to `.gitignore`. It never deletes files of deselected agents, and it leaves existing MCP config alone unless you pass `--mcp`. Every file it writes or deletes must be a real file inside the project: a symlink anywhere on the path (for example a committed `.claude/skills/seite/SKILL.md` pointing elsewhere) stops the upgrade with an error naming it.

Sites from seite 0.20 and earlier also had the `theme-builder`, `brand-identity`, and `landing-page` skills and a `design-prompts` rule. `/seite theme`, `/seite brand`, and `/seite landing` replace them. Upgrade removes copies that are still exactly as seite wrote them (checked against the content hash of every released version) and keeps edited copies, with a notice shown once. It does the same for the `AGENTS.md` sections 0.20 wrote about those skills and the dev-server REPL, leaving a one-line pointer to `/seite theme`, `/seite brand`, and `/seite landing`.

## Next steps

- [MCP Server](/docs/mcp-server): for chat apps without a shell, such as Claude Desktop
- [CLI Reference](/docs/cli-reference): every flag of `seite agent`, `seite check`, and `seite hooks`
- [Getting Started](/docs/getting-started): create your first site
