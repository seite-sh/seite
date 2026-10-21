---
title: "Validate a Static Site Before Deploy, Down to the Line"
date: 2026-11-03
description: "Validate a static site before deploy with seite check: config typos, frontmatter, shortcodes, broken links and missing assets, each traced to its source line."
tags:
  - cli
  - deployment
  - ai-agents
  - static-site-generator
draft: true
extra:
  primary_keyword: "validate a static site before deploy"
---

Your coding agent says "Done, the new docs page is live." You open the site and the sidebar is gone, two posts link to a slug that no longer exists and `seite.toml` has `minfy = true` in it. Nothing crashed. Nothing warned you. The agent just stopped.

The fix isn't more care. It's a check that runs every time, reads the whole site and tells you (or the agent) the exact file and line to change. That's what `seite check` does, and it's a fast way to validate a static site before deploy without building into `dist/` at all.

This post runs `seite check` against a site full of deliberate mistakes and shows the real output. It covers what the diagnostics mean, how to wire the check into CI and how the stop hook hands errors back to Claude Code, Codex, Cursor and OpenCode before they finish. It also covers where a dedicated link checker like lychee is still the better tool.

## How to Validate a Static Site Before Deploy

Most guides to validating a static site before deploy start and end with links. Links matter, but they're the last thing to break in a markdown site. The earlier failures are quieter:

- **Config**: a typo in `seite.toml` that the build silently ignores
- **Frontmatter**: a YAML error that drops a page or fails the build
- **Shortcodes**: Hugo-style syntax pasted into a seite site
- **Data files**: a quote left open in `data/nav.yaml`, or a list where the theme wants a map
- **Templates**: a parse error, or a render error that only shows up on some pages
- **Internal links**: a renamed slug that three posts still point at
- **Assets**: an image, script or stylesheet path that doesn't exist in the output

`seite check` covers every item on that list in one pass. It loads the config, parses every template, then runs the real build pipeline into a throwaway temporary directory. Your `dist/` folder is never created, cleaned or modified, so you can run it while the dev server is up.

```bash
seite check            # errors fail (exit 1), warnings are listed
seite check --strict   # warnings fail too
seite check --drafts   # include draft: true content
```

## Run seite check on a Broken Site

Here's a fresh site from `seite init`, with mistakes added on purpose:

- The `minify` key in `seite.toml` became `minfy`
- One docs page has an unquoted colon in its description
- A post uses Hugo shortcode syntax
- The nav data file has an unclosed quote

```text
$ seite check
content/docs/configuration.md:3:32: error[frontmatter-parse]: invalid frontmatter: mapping values are not allowed in this context
content/posts/2026-11-01-launch.md:11:1: error[shortcode-syntax]: invalid shortcode syntax: `youtube id="dQw4w9WgXcQ"`. Expected `name(args...)`
  hint: seite shortcodes take arguments in parentheses: `{{< youtube(id="dQw4w9WgXcQ") >}}`
data/nav.yaml:5:1: error[data-file-parse]: invalid YAML: found unexpected end of stream, while scanning a quoted scalar
seite.toml:48:1: warning[config-unknown-key]: unknown key `minfy` in [build] (it is ignored)
  hint: did you mean `minify`?

Error: found 3 errors, 1 warning
```

Four problems in four different kinds of file, all in one run. Line 3, column 32 is exactly where the stray colon sits. The shortcode hint hands you the corrected syntax to paste back in. The config typo is a warning, because the build still works; it just ignores the key you meant to set.

Fix those and run it again. The nav file now parses, but its shape is wrong: the theme loops over `data.nav` as a list, and the file holds a map. Every page that renders the nav fails, and `seite check` names each one along with the template involved (`template-render`, four errors).

Turn the nav into a list and the errors go away. What's left are the problems that don't stop a build:

```text
$ seite check
content/docs/configuration.md:7: warning[broken-link]: link to `../docs/intro.md` does not match any content file
  hint: relative .md links must point at a content file (path is relative to this file)
content/posts/2026-11-01-launch.md:7: warning[broken-link]: link to `/docs/getting-startd` does not match any page
  hint: did you mean `/docs/getting-started`?
content/posts/2026-11-01-launch.md:9: warning[missing-asset]: `/static/dashboard.png` does not exist in the build output
  hint: add the file under static/ or public/, or fix the path
seite.toml:48:1: warning[config-unknown-key]: unknown key `minfy` in [build] (it is ignored)
  hint: did you mean `minify`?
✓ No errors (0 errors, 4 warnings)
```

That run exits 0. Add `--strict` and the same four warnings fail it with exit code 1 and `check failed: 4 warnings (--strict treats warnings as errors)`. Strict mode is the one you want in CI and before a deploy.

## Reading the Diagnostics

Every line follows the format compilers use, so your editor and your agent already know how to parse it:

```text
file:line:col: severity[code]: message
  hint: suggestion
```

The location is as precise as seite can make it. Frontmatter and config problems carry a line and column.

Broken links and missing assets carry the line in the markdown source, not a path in generated HTML. Render errors name the content file and the template, since a render failure doesn't belong to a single line. When a fix is obvious, the `hint:` line spells it out.

The `code` in brackets is stable. seite treats it as an API: agents and scripts match on it, so existing codes don't get renamed. Here are the ones you'll see most:

| Code | Severity | What it means |
|---|---|---|
| `config-invalid` | error | `seite.toml` doesn't parse or has a wrong value type |
| `config-unknown-key` | warning | A key the schema doesn't know, with a did-you-mean hint |
| `frontmatter-parse` | error | Invalid YAML in a content file's frontmatter |
| `shortcode-syntax` | error | Malformed shortcode; Hugo-style syntax gets the seite form |
| `data-file-parse` | error | Invalid YAML, JSON or TOML in `data/` |
| `template-render` | error | A page failed to render; names the page and template |
| `broken-link` | warning (error with `--strict`) | Internal link or relative `.md` link with no target |
| `missing-asset` | warning (error with `--strict`) | Image, script, stylesheet or media file not in the output |

The full list, including `url-collision`, `data-conflict` and `i18n-partial`, is in the [seite check CLI reference](/docs/cli-reference).

## Check a Static Site for Broken Links (and Where It Stops)

If you came here to check a static site for broken links, here's the honest scope. `seite check` validates **internal** links: root-relative hrefs like `/docs/setup`, and relative links between markdown files like `[intro](../docs/intro.md)`, which the build rewrites to the published URL. It also checks every asset reference in `img`, `srcset`, `script`, stylesheet `link`, `video`, `audio` and similar tags.

It does **not** fetch external URLs. A link to `https://example.com/gone` passes.

That's a deliberate choice for a check that runs on every agent turn: it has to be fast, offline and deterministic. External checks are slow and flaky. One write-up on [Hugo link checking with lychee](https://jamieede.com/posts/automated-link-checking-on-a-hugo-site-with-lychee-and-gitea-actions/) excludes LinkedIn, X and Facebook outright because they block bots.

So keep a dedicated checker for external links. [lychee](https://github.com/lycheeverse/lychee) runs against local files or a live site and ships a GitHub Action, and linkinator does similar work in Node. A reasonable split is `seite check --strict` as the blocking gate, plus an external-link scan on a schedule or as a non-blocking CI step.

seite also isn't the only generator that checks at build time. [`zola check`](https://www.getzola.org/documentation/getting-started/cli-usage/) builds without writing to disk and fetches external links in markdown. Docusaurus throws on broken links by default during a production build ([`onBrokenLinks`](https://docusaurus.io/docs/api/docusaurus-config)). Hugo fails the build on unresolved `ref` and `relref` shortcodes; for plain markdown links, its own docs team uses a link render hook.

Where seite differs, by default, is scope and attribution. One command covers config, frontmatter, shortcodes, data files, templates, render, links and assets. Each problem points at the source line you need to edit, with a stable code.

By contrast, many link checkers read only the generated HTML. lychee can scan markdown sources too, but the common CI recipe (like the Hugo write-up above) runs it on the built output, so it reports a path in `public/` and you go hunting for the markdown that produced it.

## Use seite check in CI and Scripts

Two things make `seite check` easy to script: exit codes and `--json`.

The exit code is 0 when there are no errors, and 1 otherwise. With `--strict`, any diagnostic at all is a failure. That's enough for a CI step:

```bash
seite check --strict
```

For tooling that wants structure, `--json` prints exactly one JSON document on stdout and moves human output to stderr. Here's the strict run from above, trimmed:

```json
{
  "ok": false,
  "command": "check",
  "error": {
    "message": "check failed: 4 warnings (--strict treats warnings as errors)",
    "diagnostics": [
      {
        "severity": "warning",
        "code": "broken-link",
        "message": "link to `/docs/getting-startd` does not match any page",
        "file": "content/posts/2026-11-01-launch.md",
        "line": 7,
        "hint": "did you mean `/docs/getting-started`?"
      }
    ],
    "summary": { "errors": 0, "warnings": 4 }
  }
}
```

On success, the same list sits under `data.diagnostics` with a `summary` of error and warning counts. A script can filter on `code` without parsing English.

Multi-site workspaces work too. Run `seite check` from the workspace root to check every site, or pass `--site blog` to check one. Paths come back relative to the workspace root (`sites/blog/content/...`), so they still point at the right file. If you deploy to Cloudflare, running the check right before you [deploy the static site to Cloudflare Pages](/blog/deploy-static-site-cloudflare-pages) catches problems while they're still cheap.

## Let Your Coding Agent Validate Before It Stops

Back to the opening scene. Coding agents are good at editing markdown and bad at noticing that the build broke three files away. Telling the agent "always run the check" in AGENTS.md helps until the one turn where it forgets.

seite v0.20 moves that rule into the harness. `seite init` (and `seite upgrade` for existing sites) installs a turn-end hook for each agent you select. When the agent finishes a turn, the hook runs `seite check --hook <agent>`:

| Agent | Hook lives in |
|---|---|
| Claude Code | `.claude/settings.json` (`hooks.Stop`) |
| Codex CLI | `.codex/hooks.json` (approve it once in `/hooks`) |
| Cursor | `.cursor/hooks.json` (`hooks.stop`) |
| OpenCode | `.opencode/plugins/seite-check.js` |

If the site has errors, the hook answers in that agent's own protocol so the agent keeps working instead of stopping. Here's what Claude Code receives when a frontmatter list is left unclosed:

```json
{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"seite check found 1 error:\ncontent/docs/configuration.md:5:1: error[frontmatter-parse]: invalid frontmatter: did not find expected ',' or ']', while parsing a flow sequence\nFix these `seite check` errors, then finish. Run `seite check` to confirm."}}
```

Codex gets the same text as `{"decision":"block","reason":...}`. Cursor gets a `followup_message`, and the OpenCode plugin sends it back as a prompt.

The hook is built to stay out of the way:

- **Errors only.** Warnings never block a turn, so a broken link won't trap the agent in a loop. Use `--strict` in CI for those.
- **Once per turn.** If the hook already continued this turn (`stop_hook_active` for Claude Code and Codex, `loop_count` for Cursor, the plugin's own flag for OpenCode), it lets the agent stop.
- **Fail-open.** It always exits 0, stays silent on a clean site or outside a seite project, and caps the feedback at 40 lines.
- **Opt-out sticks.** Delete the hook and `seite upgrade` won't add it back.

There are a few limits worth knowing:

- Codex skips project hooks until you review them in `/hooks`.
- Cursor's `cursor-agent -p` mode doesn't run `stop` hooks.
- OpenCode's one-shot `opencode run` exits before plugins finish.
- The hook runs whichever `seite` is on the agent's `PATH`, so everyone on the project needs 0.20 or newer.

The [AI agent docs](/docs/agent) cover each harness in detail.

Agents can also run the check on demand. The site's [MCP server for websites](/blog/mcp-server-for-websites) exposes it as the `seite_check` tool, which takes `drafts` and `strict` and returns `ok`, a `summary` and the same diagnostics as JSON.

It's marked read-only, since it renders into a scratch directory that gets deleted. The [MCP server reference](/docs/mcp-server) lists the other 10 tools. If you're setting an agent up from scratch, [how to build a website with an AI coding agent](/blog/how-to-build-website-ai-coding-agent) walks through the whole loop.

## What Changed in seite build

The same diagnostics work landed in `seite build` in [seite v0.20.0](/changelog/v0-20-0), so a regular build is a better pre-flight check too:

- **Every broken file at once.** A bad shortcode in one post no longer hides broken frontmatter in another. You get the whole list, then fix it in one go.
- **Links point at the source.** Broken links and missing images, scripts and stylesheets name the `.md`, template or data file and line, not the generated HTML, with did-you-mean suggestions.
- **A failed build keeps `dist/`.** Full builds write to a staging directory and swap it in only when every step succeeds.

That last one is easy to see. With the unclosed frontmatter list in place, `seite build` exits 1:

```text
$ seite build
⚠ seite.toml:48:1: warning[config-unknown-key]: unknown key `minfy` in [build] (it is ignored)
  hint: did you mean `minify`?
content/docs/configuration.md:5:1: error[frontmatter-parse]: invalid frontmatter: did not find expected ',' or ']', while parsing a flow sequence

Error: found 1 error, 0 warnings
```

The previous `dist/index.html` keeps its old timestamp. A preview server or deploy script reading `dist/` still sees the last good site, not an empty folder.

So when should you use which? Use `seite check` while you're editing and in agent turns, because it writes nothing. Use `seite build --strict` when you need the output anyway, such as in a deploy job.

## FAQ

### Does seite check modify dist/?

No. It runs the full build pipeline into a temporary directory that's deleted afterwards. `dist/` and `dist-subdomains/` are never created, cleaned or written. You can run it with the dev server running, or in a read-only CI step.

### Does seite check find broken external links?

No. It checks internal links, relative `.md` links between content files and asset references, all offline. For external URLs, run a dedicated checker such as lychee or linkinator, ideally on a schedule or as a non-blocking CI step, since external sites rate-limit and block bots.

### What's the difference between seite check and seite check --strict?

Without `--strict`, only errors fail the check (exit 1). Warnings such as broken links, missing assets and unknown config keys are listed, but the exit code is 0. With `--strict`, any diagnostic fails. Use plain `seite check` while editing and `--strict` in CI.

### Will the stop hook trap my agent in a loop?

No. It blocks only on errors, never on warnings, and continues the agent at most once per turn using each harness's own loop guard. It always exits 0, so if seite itself fails, the agent just stops normally.

### Do I need seite agent for the stop hook to work?

No. The hook lives in each agent's own config (`.claude/settings.json`, `.codex/hooks.json`, `.cursor/hooks.json`, or the OpenCode plugin). It fires whenever you run that agent in the project, whether you start it yourself or through `seite agent`.

## Validate a Static Site Before Deploy, Every Time

The cheapest way to validate a static site before deploy is to make the check impossible to forget:

- Run `seite check` while you edit. It's offline and never touches `dist/`.
- Gate CI and deploys on `seite check --strict`, and match on the `code` field in `--json` output if you script around it.
- Keep lychee or linkinator for external links, on a schedule.
- Let the stop hook catch what your agent breaks before it says "done".

Already on an older site? Run `seite self-update`, then `seite upgrade` to add the hooks for your agents, then `seite check --strict` to see where you stand. Every line it prints points at the file you need to open.
