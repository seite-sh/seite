---
paths:
  - "src/mcp/**"
---
# MCP Server

`seite mcp` runs JSON-RPC 2.0 over stdio. Declared in `.mcp.json` (the only place Claude Code reads project MCP servers); `.claude/settings.json` pre-approves it via `enabledMcpjsonServers` so it starts without a prompt. `seite upgrade` migrates any legacy `mcpServers` block out of `settings.json`. Also used by Codex CLI, OpenCode, and Cursor — keep every client's constraints in mind (below).

## Architecture
Synchronous read loop on stdin, dispatches methods, writes to stdout. All logging to stderr (never stdout). No async runtime. Config is re-read before every resource/tool request.

## Protocol (`src/mcp/protocol.rs`)
- Legacy revisions via `initialize`: `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05` — echo the requested one, else answer the newest. Stored per connection (`Session`).
- Modern `2026-07-28` (stateless): version in `params._meta["io.modelcontextprotocol/protocolVersion"]`; `server/discover`; unsupported version → `-32022` with `data.supported`. Modern results get `resultType`, `_meta` serverInfo, and `ttlMs`/`cacheScope` on list/read.
- Feature gates: tool `annotations` ≥ 2025-03-26; tool `title`, `outputSchema`, `structuredContent` ≥ 2025-06-18. No capability is advertised unless implemented (no prompts/logging/subscriptions → `-32601`).
- Envelope: `jsonrpc` must be `"2.0"`, ids are string/number (null → invalid request), notifications never get a response, batches (arrays) are accepted.

## Resources (read-only)
- `seite://docs` / `seite://docs/{slug}` — embedded docs (include_str!)
- `seite://config` — `seite.toml` as JSON
- `seite://content` / `seite://content/{collection}` — content inventory
- `seite://themes` — bundled + installed themes
- `seite://trust` — trust center state (when configured)
- `seite://mcp-config` — `.mcp.json` + `.claude/settings.json`
- `resources/templates/list` advertises `seite://content/{collection}` and `seite://docs/{slug}`

## Tools (`TOOLS` table in `src/mcp/tools.rs`; new tools in `src/mcp/tools/*.rs`)
- `seite_build`, `seite_create_content`, `seite_search`, `seite_apply_theme`, `seite_lookup_docs`
- `seite_check` (read-only annotations; renders into a scratch dir like `seite check`) — returns `{ok, summary: {errors, warnings}, diagnostics}`; `ok` is false on any error, or any warning with `strict: true`
- `seite_get_page` (path or url → resolved frontmatter, body, rendered body HTML, output_path)
- `seite_update_frontmatter` (set/unset top-level keys; body preserved byte-for-byte; content dir only)
- `seite_content_stats`, `seite_list_templates`, `seite_create_collection`
- `seite_build`'s output schema includes `broken_links` and `missing_assets` (each `[{target, sources}]`, grouped by target) plus `diagnostics` (the same `Diagnostic` shape as `seite_check`); `strict: true` fails the call (`isError`) on any warning, broken link, or missing asset

Adding a tool = one `ToolDef` entry (name, title, description, input/output schema fns, annotations, handler). Rules enforced by tests:
- Tool-execution failures are `isError: true` results (text only); JSON-RPC errors only for protocol problems.
- Success: compact JSON text + `structuredContent`. Declare `outputSchema` only if EVERY success path conforms (Cursor/OpenCode reject mismatches); `test_every_output_schema_validates_real_outputs` must exercise each declared schema.
- Input schema: `{"type":"object","properties":{...},"required":[...],"additionalProperties":false}`, no combinators, every property typed, free-form objects `additionalProperties: true` (Codex/OpenAI schema conversion).
- ≤ 12 tools, `mcp__seite__<name>` ≤ 60 chars (Cursor limits). Annotations must be accurate.
- `seite_list_templates` context variables must cover every `ctx.insert("…")` in `src/build/mod.rs` (test fails otherwise).

## Files
`src/mcp/mod.rs` (transport, dispatch, instructions), `protocol.rs`, `resources.rs`, `tools.rs` + `tools/`, `content_index.rs`, `src/docs.rs` + `seite-sh/content/docs/` (embedded docs)
