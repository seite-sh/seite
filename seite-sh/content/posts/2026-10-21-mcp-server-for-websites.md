---
title: "MCP Server for Websites: Let Agents Build Your Site"
date: 2026-10-21
description: "An MCP server for websites gives coding agents typed access to config, content, templates and build. See seite's 11 tools and set it up in four agents."
tags:
  - ai
  - mcp
  - claude-code
  - static-site-generator
extra:
  primary_keyword: "MCP server for websites"
---

Search for an MCP server for websites and you mostly find servers for someone else's content. Astro runs one for Astro's documentation. Mintlify hosts one for docs sites on its platform. Directories list dozens that scrape pages or push files to a host.

Those are useful. None of them help the coding agent working on *your* site right now, the one deciding where a new post goes, what its URL will be and whether the link it just wrote points anywhere.

That's the job of the MCP server built into seite. It gives Claude Code, Codex, OpenCode and Cursor typed access to your site's configuration, content, templates and build, and a way to check their own work before they say "done." This post explains what MCP is, why reading files falls short, what each of the 11 tools does and how to set it up in each agent. It also covers when you don't need any of this.

## What Is an MCP Server? (The Short Version)

The Model Context Protocol is an open standard that Anthropic [announced on November 25, 2024](https://www.anthropic.com/news/model-context-protocol) for connecting AI tools to data and actions. An MCP *server* is a program that offers two main things to an AI client:

- **Resources**: data the agent can read, each with a URI
- **Tools**: named actions with a typed input schema the agent can call

The client (Claude Code, Cursor and so on) asks the server what it offers, shows those tools to the model and runs the calls the model picks. Messages are JSON-RPC. Local servers usually talk over stdin and stdout; remote ones use HTTP.

So an MCP server for websites is any server whose resources and tools describe a website. The interesting question is *which* website, and *whose* agent is asking.

## Four Kinds of MCP Server for Websites

The search results blur together four different things. They solve different problems, so it helps to pull them apart.

| Kind | Example | Who uses it | What it sees |
|---|---|---|---|
| Docs search | [Astro Docs MCP](https://docs.astro.build/en/guides/build-with-ai/), [Mintlify MCP](https://www.mintlify.com/docs/ai/model-context-protocol) | Readers' AI tools | A published docs index, hosted remotely |
| Hosted builder API | [Webflow MCP server](https://developers.webflow.com/mcp) | The site owner's agent, over OAuth | Your project on that platform |
| Deploy and hosting | Various listings on MCP directories | The site owner's agent | Built files and a hosting account |
| Build-aware, local | seite (`seite mcp`) | The coding agent editing the repo | Your source: config, content, templates, build |

Astro's server is a good example of the first kind. It gives your agent current answers about Astro's APIs, which helps you write Astro code, but it doesn't know anything about the site in your repo. Mintlify goes further, with a search server for readers and a separate admin server for trusted editing agents, but it's a hosted platform for documentation.

There are community projects in the fourth slot too. A [Hugo MCP server](https://mcp.so/servers/hugo-mcp?tab=tools) by an independent author can create sites and posts and manage themes. It isn't part of Hugo itself, and you install and maintain it separately.

seite's version is in the fourth row, shipped in the same binary as the generator. Nothing to install beyond seite itself, no account, free under the MIT license.

## Why Reading Files Isn't Enough

A coding agent can read `seite.toml` and `content/posts/*.md` on its own. So why bother with a server?

Because the most important facts about a site aren't written in any file. They're computed by the build. Take a post at `content/posts/2026-10-21-pricing-update.md`:

- Its date comes from the `date:` frontmatter, or from the filename when that's missing.
- Its URL depends on the collection's URL prefix, which may come from a default rather than anything in `seite.toml`.
- Its rendered HTML depends on shortcodes, templates and data files.
- Whether its links work depends on every other page in the site.

An agent reading files has to re-derive all of that, and one wrong guess is enough to break a page. The URL is the classic mistake: on seite.sh, posts live at `/blog/...` even though the folder is `content/posts`. An agent that guesses `/posts/...` writes a broken link that looks perfectly reasonable.

There's a fair counterargument: agents can run the CLI. Mario Zechner's [MCP vs CLI comparison](https://mariozechner.at/posts/2025-08-15-mcp-vs-cli/) concludes that a good CLI often beats an MCP wrapper, and that MCP earns its place when there's no easy CLI, when CLI output is too verbose or when the client has no shell. We agree with most of that. seite's CLI stays available, and the agent permissions `seite init` writes allow `seite build`, `seite check` and `seite new`.

The server earns its place by returning resolved state as data. `seite build` prints a human summary; `seite_build` returns broken links as `{target, sources}` objects.

Reading a post gives you markdown; `seite_get_page` gives you the resolved frontmatter, the real URL and the rendered body HTML in one call. That's not a thin wrapper around the CLI. It's information the CLI's text output doesn't carry.

## What seite's MCP Server for Websites Exposes

The server runs as `seite mcp` from the site directory. When a client connects, it also sends a short set of instructions telling the agent to prefer the server over reading files whenever it needs the *current* site state. Full reference is on the [MCP server docs page](/docs/mcp-server). Here's how the pieces fit together.

### Resources: Read the Site as the Build Sees It

| Resource | What the agent gets |
|---|---|
| `seite://config` | The live `seite.toml` as JSON, re-read on every request |
| `seite://content` | Every collection with item counts |
| `seite://content/{collection}` | Each item's title, slug, real URL, source path, language, draft flag, date, tags and description |
| `seite://themes` | Bundled and installed themes |
| `seite://docs`, `seite://docs/{slug}` | seite's own documentation, embedded in the binary |
| `seite://trust` | Trust center state (only when the site has one) |
| `seite://mcp-config` | Each coding agent's project config that exists |

The collection listing is the one agents use most. It answers "does this post already exist?" and "what's the URL of the pricing page?" without guessing. A file that fails to parse shows up with its `parse_error` instead of silently disappearing.

### Tools That Look

Six of the 11 tools are read-only:

- `seite_search` finds content by keyword across titles, descriptions, tags and body text, ranked in that order, including drafts.
- `seite_get_page` inspects one page by source path or URL: resolved frontmatter, word count, markdown body and rendered HTML.
- `seite_content_stats` is a health report: drafts, missing descriptions, untagged dated posts, future-dated items, unparseable files and missing translations.
- `seite_list_templates` returns template overrides, blocks in the active `base.html`, Tera context variables, shortcodes with their parameters and data file keys. Agents call it before touching templates.
- `seite_lookup_docs` searches seite's docs by keyword or topic.
- `seite_check` reports every problem in the site: config syntax and unknown keys, frontmatter, shortcodes, data files, templates, rendering, broken internal links and missing assets. It renders into a temporary directory and never touches `dist/`.

### Tools That Change Things

The other five write files:

- `seite_create_content` creates a page with valid frontmatter in the right place and returns its path and URL. It refuses to overwrite an existing file unless the agent passes `overwrite: true`.
- `seite_update_frontmatter` sets or removes frontmatter keys and keeps the markdown body byte-for-byte. Usually only the changed lines move, so comments and key order survive. If the YAML is too unusual for targeted edits, it rewrites the whole frontmatter instead and says so in the result's `notes`.
- `seite_build` builds to the output directory and returns stats, warnings, broken links, missing assets and diagnostics. With `strict: true` any warning fails the call.
- `seite_apply_theme` swaps in a bundled or installed theme and backs up a customized `base.html` first.
- `seite_create_collection` adds a preset collection (posts, docs, pages, changelog, roadmap or trust) to `seite.toml` and creates its directory.

### Annotations, Structured Output and Errors

Every tool carries MCP annotations that tell the client how careful to be. The six read-only tools say so. `seite_create_content` is the only one marked destructive, because `overwrite: true` can replace a file. `seite_build`, `seite_update_frontmatter` and `seite_apply_theme` are marked idempotent: running them twice with the same input changes nothing more.

Results come back as JSON text and, for clients on the 2025-06-18 protocol or newer, as `structuredContent`. Ten of the 11 tools also declare an `outputSchema`, so a client can validate the result instead of parsing prose. The [MCP tools specification](https://modelcontextprotocol.io/specification/2025-06-18/server/tools) requires servers to return results that conform to a declared schema.

Failures are results too. A bad argument, a missing site or a file that already exists comes back with `isError: true` and a message that says what to do next. The agent reads it and retries instead of stalling on a protocol error.

## A Real Session: Create, Check, Fix

Here's what the agent sees. The JSON below is real output from seite 0.20.1 against a fresh `seite init` site, trimmed to the `structuredContent` field. The narration around it is illustrative.

Say you ask for a short post announcing a pricing change. First, the agent checks whether one exists:

```json
{
  "query": "pricing",
  "total": 0,
  "returned": 0,
  "results": []
}
```

That's the result of `seite_search` with `{"query": "pricing"}`. No pricing post yet, so it calls `seite_create_content` with the collection, title, tags and body. Its draft links to `/pricing`, a page this site doesn't have:

```json
{
  "path": "content/posts/2026-10-21-pricing-update.md",
  "url": "/posts/pricing-update",
  "draft": false,
  "overwritten": false
}
```

Then it checks its work with `seite_check`:

```json
{
  "ok": true,
  "summary": { "errors": 0, "warnings": 1 },
  "diagnostics": [{
    "severity": "warning",
    "code": "broken-link",
    "message": "link to `/pricing` does not match any page",
    "file": "content/posts/2026-10-21-pricing-update.md",
    "line": 9
  }]
}
```

Note `ok: true`. A broken internal link is a warning by default; `strict: true` turns it into a failure. The agent has the file and line either way, so it fixes the link (or asks you whether a pricing page should exist) and runs the check again.

If the agent retries the create call by mistake, nothing is lost:

```text
content/posts/2026-10-21-pricing-update.md already exists; refusing to overwrite it.
Edit the existing file or choose a different title. Pass "overwrite": true to replace it,
or use a different slug.
```

That's the whole loop: look before writing, write through a tool that knows the rules, check, fix. No dist/ folder was touched until the agent chose to build.

## Set Up an MCP Server for Websites in Claude Code, Codex, OpenCode and Cursor

You don't write any of this config by hand. `seite init` registers the server for the agents you select. Interactively, it preselects the agents it finds installed (all four if it finds none); `--agents claude,codex,opencode,cursor` or a non-interactive run sets up all four. And `seite upgrade` adds it to existing sites without dropping servers you already have. [v0.20.0](/changelog/v0-20-0) is the release that brought Codex, OpenCode and Cursor support and moved Claude Code's entry into `.mcp.json`.

| Agent | Config file seite writes | One-time step |
|---|---|---|
| Claude Code | `.mcp.json` (pre-approved through `enabledMcpjsonServers` in `.claude/settings.json`) | Open the project once and accept the workspace trust prompt; approve the server if asked; check with `/mcp` |
| Codex CLI | `.codex/config.toml` (`[mcp_servers.seite]`) | Trust the project when Codex asks; untrusted projects ignore the file. Check with `/mcp` |
| OpenCode | `opencode.json` (`mcp.seite`) | None; it starts automatically |
| Cursor | `.cursor/mcp.json` (+ `.cursor/cli.json` permissions) | Approve it in Cursor's MCP settings or run `cursor-agent mcp enable seite`; the CLI also needs workspace trust |

Starting fresh looks like this:

```bash
seite init mysite --title "My Site" --collections posts,docs,pages --deploy-target cloudflare --agents claude,codex,opencode,cursor
cd mysite
claude          # or codex, opencode, cursor-agent
```

New to seite? The [getting started guide](/docs/getting-started) covers installing it first. To pick up the server on a site made before v0.20, run `seite upgrade` in the site directory. You can also launch any of the four with your site's live context through `seite agent --with codex` (or `opencode`, `cursor`); the [seite agent docs](/docs/agent) cover the flags and permissions.

## The Stop Hook: The Agent Checks Its Own Work

Tools only help if the agent calls them, and nothing in MCP forces it to.

So `seite init` also installs a turn-end hook for each agent: Claude Code's `hooks.Stop` in `.claude/settings.json`, `.codex/hooks.json`, `.cursor/hooks.json` and an OpenCode plugin. When the agent finishes, the hook runs `seite check --hook <agent>`. If the site has errors, it hands them back:

```text
seite check found 1 error:
content/posts/2026-10-21-launch-notes.md:3:17: error[frontmatter-parse]: invalid frontmatter: ...
Fix these `seite check` errors, then finish. Run `seite check` to confirm.
```

The rules are deliberately narrow. Only errors block; warnings like that `/pricing` link never do.

The hook sends the agent back at most once per turn and always exits with status 0, so the agent can't get stuck in a fix-and-retry loop. Delete the hook to opt out; `seite upgrade` won't re-add it. Codex asks you to approve hooks once in `/hooks`.

## Limits, and When You Don't Need This

Be clear about what this server is and isn't.

**It's local.** `seite mcp` speaks stdio only. The agent's client starts it as a subprocess in your site directory. There's no remote endpoint, no port and no account.

**It needs seite on your PATH.** The configs call `seite mcp` by name. Everyone who runs an agent on the site needs seite 0.20 or newer installed for the server and the stop hook to work.

**You'll see approval prompts.** That's by design. The MCP spec says clients should keep a human in the loop who can deny tool calls, and that clients must treat annotations as untrusted unless the server is trusted. Claude Code, Codex and Cursor each ask once, in their own way, and the setup table lists where. OpenCode starts the server without asking.

**It's for building the site, not for visitors.** ChatGPT, Perplexity and other AI tools reading your published site never talk to this server. They read your HTML, your markdown copies and your llms.txt, which seite generates on every build. If your goal is AI discoverability rather than agent tooling, start with [what llms.txt is and why it matters](/blog/what-is-llms-txt).

**It doesn't replace review.** A clean `seite_check` means the site builds and its links resolve. It doesn't mean the copy is right.

## FAQ

### What is an MCP server for websites?

It's a Model Context Protocol server that exposes a website's data and actions to AI tools as resources and typed tools. Some serve published docs to readers' AI tools. seite's serves the site's source to the coding agent building it: config, content, templates, build and checks.

### Does my website need an MCP server?

Not for visitors. For AI search and chat tools, llms.txt and markdown copies of your pages do the work. An MCP server pays off when a coding agent edits your site regularly, because it stops the agent from guessing URLs, frontmatter and build results.

### Is MCP better than letting the agent run the CLI?

Neither wins everywhere. A CLI is often enough for simple commands. MCP helps when the agent needs structured results it can act on, such as broken links with source files and lines, resolved URLs or a page's rendered HTML. seite gives agents both.

### Does seite's MCP server only work with Claude Code?

No. `seite init` registers it for Claude Code, Codex CLI, OpenCode and Cursor. Any other MCP client can run `seite mcp` from the site directory, since it's a standard stdio server.

### Can ChatGPT or Perplexity use it on my live site?

No. It runs locally while you build and isn't deployed with your site. Visitors' AI tools read your published llms.txt, llms-full.txt and per-page markdown, which seite builds automatically.

### Is it safe to let an agent write files through it?

Writes are scoped to the site and clearly labelled. Creating content refuses to overwrite unless asked, frontmatter edits leave the body untouched, theme changes back up a customized template and Claude Code, Codex and Cursor each ask you to approve the project or server once.

## Try seite's MCP Server for Websites

The short version:

- Most MCP servers for websites serve published content. seite's serves the site you're building.
- Its 11 tools and live resources give agents resolved URLs, frontmatter and build results instead of guesses.
- `seite_check` and the stop hook mean the agent finds its own broken links and bad frontmatter before you do.
- It's local, free, ships in the seite binary and is set up for four coding agents by `seite init`.

To try it, run `seite init` (or `seite upgrade` on an existing site), open the project in your agent and ask it to list your posts. For a full walkthrough from empty folder to deployed site, follow the guide to [building a website with an AI coding agent](/blog/how-to-build-website-ai-coding-agent). For the bigger picture of why this matters, read about [what makes an AI static site generator different](/blog/ai-static-site-generator).
