---
paths:
  - "src/mcp/**"
---
# MCP Server

`seite mcp` runs JSON-RPC 2.0 over stdio. Aimed at MCP hosts without a shell (Claude Desktop chat: `seite mcp --dir /path`). Coding agents use the CLI; the server is only declared in their configs with `seite init --mcp` / `seite upgrade --mcp` (`.mcp.json` + `enabledMcpjsonServers`, `.cursor/mcp.json`, `.codex/config.toml`, `opencode.json`, `.gemini/settings.json`). Keep every client's constraints in mind (below).

## Architecture
Synchronous read loop on stdin, dispatches methods, writes to stdout. All logging to stderr (never stdout). No async runtime. Config is re-read before every resource/tool request.

HTTP transport: the dev server's `POST /__seite/mcp` (`src/server/dev.rs`) calls `mcp::handle_http_message` (stateless: legacy version from the `MCP-Protocol-Version` header, one message or batch per POST). Its `ServerState` is one `Arc<Mutex<ServerState>>` per server with `dev: Some(DevState)`; since `dispatch` reloads `seite.toml` before every resource/tool request, nothing has to reload it after a rebuild. Guards (loopback Host + Origin, JSON content type, 1 MB, disabled on a non-loopback bind, no CORS headers) are in `dev::mcp_guard`. The workspace server has no MCP endpoint (404 naming `seite serve --site <name>` / `seite mcp --dir <site>`).

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
- `resources/templates/list` advertises `seite://content/{collection}` and `seite://docs/{slug}`

## Tools (`TOOLS` table in `src/mcp/tools.rs`; new tools in `src/mcp/tools/*.rs`)
- `seite_build`, `seite_create_content`, `seite_search`, `seite_apply_theme`, `seite_lookup_docs`
- `seite_audit` (read-only; `categories`, `top` (50), `min_severity` (warning)) — `audit::run` on a scratch build + `Report::limit`, returns `{summary, findings, omitted}` (summary counts everything)
- `seite_check` (read-only annotations; renders into a scratch dir like `seite check`) — returns `{ok, summary: {errors, warnings}, diagnostics}`; `ok` is false on any error, or any warning with `strict: true`
- `seite_get_page` (path or url → resolved frontmatter, body, rendered body HTML, output_path). Renders through the build's own `build::render_item_body()` (shortcodes → math → markdown, with `shortcode_site_context`/`shortcode_page_context`), so it can't drift from `seite build`; `render_error` lists every shortcode diagnostic with file lines
- `seite_update_frontmatter` (set/unset top-level keys; body preserved byte-for-byte; content dir only). `edit_frontmatter_text()` in `src/mcp/tools/page.rs` applies changes as line edits on top-level entries (key line + indented/compact-list continuation; values serialized with serde_yaml), so comments and untouched keys stay byte-identical; unchanged values aren't rewritten. The edited text must parse back to exactly the intended mapping, else it falls back to full re-serialization (with a `notes` entry). Validation (title, types, dated-collection date) runs before anything is written.
- `seite_content_stats`, `seite_list_templates`, `seite_create_collection`
- Runtime tools (`src/mcp/tools/runtime.rs`, no outputSchema): `seite_get_errors` (last build + browser errors), `seite_get_preview_url` (`path`|`url` → preview URL), `seite_list_routes` (`.html` files in the output dir; `preview_base` when a server runs), `seite_reload` (rebuild via the server). With `state.dev` they read the in-process `DevState`; under stdio they use `server::remote` (`/__seite/status`; `seite_reload` proxies `tools/call` to `/__seite/mcp`), and with no server fail with "no dev server running; start it with `seite serve`".
- `seite_build`'s output schema includes `broken_links` and `missing_assets` (each `[{target, sources}]`, grouped by target) plus `diagnostics` (the same `Diagnostic` shape as `seite_check`); `strict: true` fails the call (`isError`) on any warning, broken link, or missing asset

Adding a tool = one `ToolDef` entry (name, title, description, input/output schema fns, annotations, handler). Rules enforced by tests:
- Tool-execution failures are `isError: true` results (text only); JSON-RPC errors only for protocol problems.
- Success: compact JSON text + `structuredContent`. Declare `outputSchema` only if EVERY success path conforms (Cursor/OpenCode reject mismatches); `test_every_output_schema_validates_real_outputs` must exercise each declared schema.
- Input schema: `{"type":"object","properties":{...},"required":[...],"additionalProperties":false}`, no combinators, every property typed, free-form objects `additionalProperties: true` (Codex/OpenAI schema conversion).
- ≤ 16 tools (12 site + 4 runtime), `mcp__seite__<name>` ≤ 60 chars (Cursor limits). Annotations must be accurate.
- `seite_list_templates` context variables must cover every `ctx.insert("…")` in `src/build/mod.rs` (test fails otherwise).

## Files
`src/mcp/mod.rs` (transport, dispatch, instructions), `protocol.rs`, `resources.rs`, `tools.rs` + `tools/`, `content_index.rs`, `src/docs.rs` + `seite-sh/content/docs/` (embedded docs)
