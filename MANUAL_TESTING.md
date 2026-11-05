# Manual Testing: 0.21 local release checklist

Things `cargo test` can't cover: real browsers, real agent CLIs, upgrade from a released binary. Run top to bottom; every step is a copy-pasteable block followed by `expect:`. Nothing here pushes, tags or deploys unless a section says "only after you decide to deploy".

## 0. Build and install into a throwaway prefix

```sh
export REPO=$PWD                         # the checkout under test
cd $REPO && cargo build --release        # build first: rustup/cargo need your real HOME
export REAL_HOME=$HOME                   # for any later cargo command
export T=$(mktemp -d); export HOME=$T/home; mkdir -p $HOME $T/prefix/bin
export SEITE_NO_UPDATE_CHECK=1           # no background update check
cp $REPO/target/release/seite $T/prefix/bin/ && export PATH=$T/prefix/bin:$PATH
hash -r; seite --version; which seite
```
(`./local-install.sh` does the same with `INSTALL_DIR=$T/prefix/bin ./local-install.sh`.)
expect: `seite 0.21.0`, path under `$T/prefix/bin`. Trust list, tailwind cache and config now live under the fake `$HOME`. Use the same shell for everything below (or re-export `T`, `HOME`, `PATH`). Real agent CLIs need your real logins: run section 2 in a second shell with your real `HOME` and just `export PATH=$T/prefix/bin:$PATH`.

## 1. Fresh site, every starter, all seven agents

```sh
cd $T
for s in default docs landing blank; do
  seite init site-$s --title "Site $s" --deploy-target github-pages \
    --collections posts,docs,pages --theme $s --agents claude,codex,opencode,cursor,gemini,copilot,antigravity -y
  (cd site-$s && seite build && seite check --strict && seite audit)
done
```
expect: four sites, no prompts; build, `check --strict` exit 0; audit prints a score and findings (no crash). Blank starter may have audit notices; note any errors.

```sh
cd $T/site-default
ls -a . .claude .claude/skills .agents .agents/rules .cursor .gemini .github .opencode .seite
wc -c AGENTS.md; seite hooks status; seite context | tail -15
```
expect: AGENTS.md about 4 KB (seven agents), CLAUDE.md/GEMINI.md `@AGENTS.md` shims, `.claude/{settings.json,launch.json,rules,skills/seite}`, `.cursor/{cli.json,rules/*.mdc}`, `opencode.json`, `.opencode/{commands,plugins}`, `.gemini/{settings.json,commands/seite.toml}`, `.github/{hooks/seite.json,instructions}`, `.agents/{skills/seite,rules,hooks.json}` (Antigravity CLI), `.seite/docs/` (gitignored). Hooks status shows both tiers for all seven. `seite context` ends with `NAME: message` directives.

### Dev server, live reload, overlay, console forwarding
```sh
cd $T/site-default && seite serve --no-repl   # terminal A; note the URL (or --port 3000)
```
```sh
cd $T/site-default && seite preview; cat .seite/dev.json   # terminal B
seite serve --no-repl                                         # second serve: prints running URL, exits
```
expect: preview prints URL, last build OK, no browser errors; second serve exits without starting another.

In the browser (open the printed URL):
- edit `content/posts/*.md` body, save: page reloads by itself (SSE `/__seite/events`).
- change a frontmatter field to garbage (e.g. `date: not-a-date`), save: error overlay with file:line; revert: overlay clears and page reloads.
- DevTools console: `console.error("manual-test-boom")`; expect it echoed in terminal A, and `seite preview` (terminal B) lists it under browser errors. `seite preview --clear-browser-errors` then `seite preview`: list empty.
- `curl -s localhost:PORT/__seite/status`: JSON; only works on loopback.
- Ctrl-C terminal A: `.seite/dev.json` is gone.

### Accept: text/markdown against serve
```sh
cd $T/site-default && seite serve --no-repl --port 3999 &
sleep 3; curl -sI localhost:3999/ | grep -i vary
curl -s -H 'Accept: text/markdown' localhost:3999/posts/ | head -5
curl -sI localhost:3999/posts/index.md | grep -i content-type
curl -s localhost:3999/sitemap.md | head; curl -s localhost:3999/robots.txt | grep -i "claude\|Content-Signal"
kill %1
```
expect: `Vary: Accept`; markdown body starting with frontmatter then the `> Index of this site for agents:` line; `text/markdown; charset=utf-8`; sitemap.md lists pages; robots has Claude-SearchBot allowed, ClaudeBot blocked, Content-Signal line. Browser `*/*` still gets HTML. View-source of any page: `rel="alternate" type="text/markdown"` href resolves (200).

## 2. Agents end-to-end (interactive, your logins)

Per agent, in a fresh copy of the site (`cp -r $T/site-default $T/agent-<name>`), `cd` there, start the agent (or `seite agent --with <name> "<prompt>"`). Prompts, one session each:
1. `write a post about local-first software`
2. `add a docs page for installation`
3. `change the theme accent color`
4. `fix all check errors` (first break two things: a bad `date:` and a `[[broken](/nope)` link)

Pass = finished without permission prompts for the pre-approved commands (`seite build|check|new|serve|audit|preview|og preview|context|docs|page|search|stats|templates|frontmatter|explain|hooks`, `seite theme apply|list`, edits under `content/ templates/ static/ data/`); expected prompts are only the agent's one-time step from AGENTS.md → Agent setup and commands outside that list (`git`, `seite deploy`, …); `seite check --strict` clean afterwards; the `/seite` skill was used (Codex: `$seite`); post-edit hook fired after an edit (introduce a bad frontmatter field and see the agent get the error immediately) and the turn-end hook fired once; result looks right in `seite serve`.

```sh
seite agent --with claude   "write a post about local-first software"    # Claude Code
seite agent --with codex    "..."    # Codex
seite agent --with antigravity "..."  # Antigravity CLI (agy; headless needs a user allow rule for `seite check`)
seite agent --with gemini   "..."    # Gemini CLI (free tier)
seite agent --with opencode "..."    # OpenCode with a free/local model
seite agent --with cursor   "..."    # Cursor CLI
seite agent --with copilot  "..."    # GitHub Copilot CLI
seite agent --with claude --once "list the collections"   # -p path
```
Also: inside an agent session `seite agent` must refuse to nest; `seite agent --auto-approve` without `--once` or off a TTY must not add approve flags.
Record per agent: pass/fail, prompts seen, hooks fired (Y/N + tier), skill used (Y/N).

### Skill without `seite init`
```sh
mkdir $T/plain && cd $T/plain && git init -q && cp -r $T/site-default/{seite.toml,content,templates,static} . 2>/dev/null
npx skills add $REPO            # local path; expect skills/seite picked up
```
In Claude Code, from the same dir:
```text
/plugin marketplace add /absolute/path/to/this/checkout
/plugin install seite@seite
```
expect: plugin installs; `/seite check` works; hooks run only inside a site (silent in a non-site dir); with `.claude/settings.json` already holding seite hooks the plugin's hooks stand down. Check `git status` in `$REPO` is clean after `HOME=$REAL_HOME cargo run -- skill export . --format repo` (drift test) if you changed anything.

## 3. Upgrade from 0.20.x

```sh
HOME=$REAL_HOME cargo install seite --version 0.20.2 --locked --root $T/old     # or the released installer
cd $T && $T/old/bin/seite init old --title Old --deploy-target github-pages --collections posts,docs,pages -y
cd old && $T/old/bin/seite build && git init -q && git add -A && git commit -qm old
ls -a .claude/skills; wc -c AGENTS.md
```
expect: 0.20.2 layout (theme-builder, brand-identity, landing-page skills; long AGENTS.md; design-prompts rule). Then with the 0.21 binary:
```sh
seite upgrade --check; echo "exit=$?"
seite upgrade -y
git status --short; ls -a .claude/skills .claude/rules .agents 2>&1; wc -c AGENTS.md; seite hooks status
seite check --strict && seite build
```
expect: `--check` exits 1 (outdated); upgrade removes the unmodified legacy skills and design-prompts rule, prunes only the legacy AGENTS.md sections it owns (retired skills, serve REPL) when unedited — the 0.20 Commands / MCP Server sections stay, so AGENTS.md is still long (stale; `seite upgrade --agent` trims it) —, adds the `seite` skill + references, keeps the stored 0.20 agent selection (claude, codex, opencode, cursor: no agents added, no gemini or copilot files; one-time notice with the `seite upgrade --agents` command) and adds their post-edit hooks next to the existing turn-end hooks, writes `.seite/docs/` (gitignored), no MCP files added, no `[og]`/`search` added (one-time notices only). A second `seite upgrade -y` changes nothing.

```sh
# edited legacy files must survive
git checkout -q . && git clean -fdq; echo "# my edit" >> .claude/skills/theme-builder/SKILL.md
seite upgrade -y; ls .claude/skills/theme-builder     # expect: kept
seite upgrade --agents claude,codex,opencode,cursor,gemini,copilot,antigravity -y; ls .github/hooks .github/instructions .agents
seite hooks off --tier post-edit; seite upgrade -y; seite hooks status   # expect: post-edit stays off
seite hooks on
seite upgrade --agent --json | head -c 600       # brief in data.brief; outside a TTY nothing is launched
seite upgrade --agent                            # from a terminal: writes .seite/upgrade-brief.md, starts claude (read-only look; quit it)
```
expect: edited skill kept; copilot files added; user-removed tier stays removed; brief lists release notes since 0.20.2, no auto-approve flags.

## 4. Features

Use `cd $T/site-default` (add to `seite.toml` as noted; `seite check` after each).

```sh
# search
grep -n search seite.toml; seite build -v | grep -i search; ls dist/search | head; seite search "welcome"
```
expect: `search = "full"`, shards under `dist/search/`. In the served site, the search box finds a word from a post body.

```sh
# OG cards
seite og preview /posts/$(ls content/posts | head -1 | sed 's/^[0-9-]*//;s/\.md$//') --template bold; open .seite/cache/og-preview.png
seite og preview / ; ls dist/og | head
```
expect: PNG with title/accent; `/` renders the site card; re-run build: `seite build --json | grep og_cards` shows `cached` > 0.

```sh
# scheduled post, series, taxonomies
seite new post "Future" --tags a,b ; sed -i.bak "s/^date:.*/date: 2099-01-01/" content/posts/*future*.md
seite build | grep -i scheduled; seite build --future; seite serve --no-repl &  # serve shows future posts
# by hand: add `series: "Demo"` (+ `series_order: 1`/`2`) to two posts' frontmatter
printf '\n[[taxonomies]]\nname = "categories"\n' >> seite.toml   # add `categories: [x]` to a post
seite build && ls dist/series dist/categories; kill %1
```
expect: "1 scheduled post hidden; next goes live ..." (needs `[build] schedule = true`, which init writes); future build includes it; `/series/demo/` and `/categories/x/` exist; "Part 1 of 2" box on a post.

```sh
# schema: break a field
cat >> seite.toml <<'EOT'

[collections.fields.rating]
type = "integer"
min = 1
max = 5
EOT
```
Add `rating: 9` to a post. `seite check` expect: `frontmatter-schema` error with file:line; `seite check --file content/posts/<that>.md` same; `seite frontmatter set content/posts/<that>.md rating=9` refused; `rating=3` ok; `seite explain frontmatter-schema` prints docs.

```sh
# external links
echo '[x](https://example.invalid/nope) [y](https://example.com)' >> content/pages/about.md
seite check --external; ls .seite/cache/links.json
```
expect: `external-link-broken`/`-unreachable` for the .invalid host; example.com OK; second run uses cache.

```sh
# Tailwind
cd $T && seite init tw --title TW --deploy-target github-pages --collections posts,pages -y && cd tw
seite theme new --tailwind; seite trust --list; seite build -v | tail -5; ls dist/static/css
seite trust --revoke; seite build; echo "exit=$?"
```
expect: first build downloads the pinned Tailwind v4.3.3 (network, SHA-256 verified; or set `SEITE_TAILWIND_BIN`), CSS under `dist/static/css/` fingerprinted, classes added to a template show after reload in `serve`; after `--revoke` build fails with `tailwind-untrusted`; `seite trust` re-enables; `seite check` and `seite audit` never run Tailwind. `SEITE_TRUST_PROJECT=1 seite build` works untrusted.

```sh
# import (small samples; --dry-run first)
cd $T && seite init imp --title Imp --deploy-target github-pages --collections posts,pages -y && cd imp
seite import wordpress /path/to/export.xml --dry-run && seite import wordpress /path/to/export.xml
seite import ghost /path/to/ghost.json --dry-run --site-url https://old.example.com
seite import jekyll /path/to/jekyll-site --dry-run
seite import hugo /path/to/hugo-site --dry-run   # add --drafts, --no-images, --allow-host cdn.example.com as needed
cat .seite/cache/import-report.md; seite check --strict; seite build
```
expect: dry-run writes nothing; real run writes content under `content/`, images under `static/images/imported/<slug>/`, a report; re-run skips existing (use `--force` to overwrite); `check --strict` clean or only report-listed items. Hugo/Jekyll starters: `hugo new site` / `jekyll new` outputs are fine samples.

```sh
# CMS, shortcodes, deploy dry-run
cd $T/site-default
seite cms setup pages-cms; seite cms setup decap; cat .pages.yml | head; ls static/admin
seite cms setup decap     # expect: refuses to overwrite; --force backs up under .seite/cache/backups/
printf '\n[comments]\nprovider = "giscus"\nrepo = "o/r"\nrepo_id = "R_x"\ncategory = "General"\ncategory_id = "DIC_x"\n\n[newsletter]\nprovider = "buttondown"\nusername = "demo"\n' >> seite.toml
printf '\n{{< comments() >}}\n\n{{< newsletter() >}}\n' >> content/posts/$(ls content/posts | head -1)
seite check --strict && seite build && grep -l "giscus\|buttondown" dist/posts/*.html | head -2
seite deploy --dry-run; seite deploy --dry-run --target cloudflare; seite deploy --dry-run --target netlify
```
expect: Pages CMS `.pages.yml` / Decap `static/admin/{index.html,config.yml}` match collections and schema; giscus/buttondown markup in page; every dry-run lists steps and makes no commit/push/network call (`git status` unchanged, no `dist` upload).

Also quick: `seite skill install seomachine` (pinned, verifies hashes, writes into every agent's skills dir; `seite skill list`, `seite skill remove seomachine` keeps edited files), `seite skill install impeccable` (asks first; only if you want npm fetched), `seite explain <code>` for a code from `check`, `seite theme apply <gallery theme>` then `seite build`.

## 5. Only after you decide to deploy (needs your approval, throwaway projects)

```sh
cd $T/site-default
seite deploy --setup               # per target; creates real projects
seite deploy --preview --verify    # free-tier preview, never production
```
- Cloudflare Pages preview: `curl -H 'Accept: text/markdown' <preview>/posts/` returns markdown only with `[deploy] markdown_negotiation = true` (or `[access]`); `_worker.js`/`_routes.json` present; `[access]` password flow, `//` path gives 400.
- Netlify and GitHub Pages previews: site loads, HTML only for `Accept: text/markdown` (expected), `robots.txt`, `llms.txt`, `sitemap.md`, `/og/*.png`, search work.
- Run Lighthouse/PageSpeed on the preview of each starter; run the impeccable detector across bundled themes (`seite theme apply <name>` for each of the 11).

## Cleanup

```sh
pkill -f "seite serve"; rm -rf $T
```

---

# Appendix: standing checks (carried over from earlier releases, still valid)

Tests that require a real browser or deployed environment and can't be covered by `cargo test`.

## Analytics & Cookie Consent Banner

### Google Analytics 4 (direct)

- [ ] Add `[analytics]` with `provider = "google"` and a real `G-XXXXXXX` ID to `seite.toml`
- [ ] Run `seite build && seite serve`, open in browser
- [ ] Verify GA4 script tag appears in page source `<head>`
- [ ] Verify real-time traffic shows up in Google Analytics dashboard
- [ ] Verify no cookie consent banner is shown

### Google Analytics 4 (with consent)

- [ ] Set `cookie_consent = true` in `[analytics]`
- [ ] Run `seite build && seite serve`, open in browser
- [ ] Verify consent banner appears at bottom of page on first visit
- [ ] Click "Decline" — banner disappears, no analytics script loaded
- [ ] Refresh page — banner stays hidden (localStorage persists)
- [ ] Clear localStorage, refresh — banner reappears
- [ ] Click "Accept" — banner disappears, analytics script loads dynamically
- [ ] Refresh page — no banner, analytics loads automatically
- [ ] Verify real-time traffic shows up in GA dashboard only after accepting

### Google Tag Manager

- [ ] Set `provider = "gtm"` with a real `GTM-XXXXXXX` ID
- [ ] Verify GTM script loads in `<head>`
- [ ] Verify `<noscript>` iframe appears after `<body>` (direct mode)
- [ ] With `cookie_consent = true`, verify no `<noscript>` tag (consent mode)
- [ ] Verify GTM debug mode shows container loading

### Plausible Analytics

- [ ] Set `provider = "plausible"` with your domain as `id`
- [ ] Verify `<script defer data-domain="..." src="https://plausible.io/js/script.js">` in source
- [ ] Verify pageview appears in Plausible dashboard
- [ ] With custom `script_url` (self-hosted), verify it uses the custom URL

### Fathom Analytics

- [ ] Set `provider = "fathom"` with a real site ID
- [ ] Verify `<script src="https://cdn.usefathom.com/script.js" data-site="..." defer>` in source
- [ ] Verify pageview appears in Fathom dashboard

### Umami Analytics

- [ ] Set `provider = "umami"` with website ID and `script_url`
- [ ] Verify script tag uses the custom `script_url` and `data-website-id`
- [ ] Verify pageview appears in Umami dashboard

### Cookie Banner UI/UX

- [ ] Banner is visible and readable on desktop (1440px+)
- [ ] Banner is visible and readable on mobile (375px)
- [ ] Banner doesn't overlap or hide page content
- [ ] "Accept" and "Decline" buttons are keyboard-focusable (Tab key)
- [ ] Focus rings are visible on buttons
- [ ] Screen reader announces banner as dialog (`role="dialog"`)
- [ ] Banner works with dark theme applied
- [ ] Banner works with all 11 bundled themes (see `seite theme list`)

### Edge Cases

- [ ] Build with no `[analytics]` section — no scripts injected, no banner
- [ ] Build with invalid provider name — config parse error (not a silent failure)
- [ ] Multiple `seite build` runs — analytics injected exactly once (not duplicated)
- [ ] Works with `seite serve` live reload (rebuilt pages still have analytics)
- [ ] Works in workspace mode (`seite build --site blog`)

## Trust Center / Compliance Hub

### Init scaffolding (interactive)

- [ ] Run `seite init trustsite --collections posts,pages,trust` — triggers trust prompts
- [ ] Verify company name prompt appears and accepts input
- [ ] Verify framework multi-select shows all 7 options (SOC 2, ISO 27001, GDPR, HIPAA, PCI DSS, CCPA, SOC 3)
- [ ] Verify section multi-select shows all 7 options with sensible defaults
- [ ] Verify per-framework status prompt appears (Active / In Progress / Planned) for each selected framework
- [ ] After completion, verify directory structure:
  - `data/trust/certifications.yaml`
  - `data/trust/subprocessors.yaml`
  - `data/trust/faq.yaml`
  - `content/trust/security-overview.md`
  - `content/trust/vulnerability-disclosure.md`
  - `content/trust/certifications/<framework>.md` per selected framework
  - `templates/trust-item.html`
  - `templates/trust-index.html`

### Init scaffolding (non-interactive / CI)

- [ ] Run `seite init site --collections posts,pages,trust --trust-company "Acme" --trust-frameworks soc2,iso27001 --trust-sections overview,certifications,subprocessors,faq,disclosure` — no prompts
- [ ] Verify same directory structure as interactive mode
- [ ] Verify `seite.toml` has `[trust]` section with company and frameworks
- [ ] Verify AGENTS.md has `## Trust Center` section with correct company name

### Build and render

- [ ] Run `seite build` on a trust-enabled site — builds successfully
- [ ] Open `/trust/` in browser — hub page renders with:
  - Hero section with company name
  - Certification cards with status badges (green=active, yellow=in_progress, gray=planned)
  - Content sections for each trust page
  - Subprocessor table (if data exists)
  - FAQ accordion (if data exists)
- [ ] Click a certification card — navigates to `/trust/certifications/<slug>`
- [ ] Certification detail page shows status badge, auditor, scope, dates
- [ ] Individual trust pages (`/trust/security-overview`, `/trust/vulnerability-disclosure`) render correctly
- [ ] Breadcrumb navigation works (Trust Center > Page Title)

### Theme compatibility

- [ ] Trust center renders correctly with `default` theme
- [ ] Trust center renders correctly with `minimal` theme
- [ ] Trust center renders correctly with `dark` theme
- [ ] Trust center renders correctly with `docs` theme
- [ ] Trust center renders correctly with `brutalist` theme
- [ ] Trust center renders correctly with `bento` theme
- [ ] Status badges are visually distinct across all themes
- [ ] Subprocessor table is responsive (scrollable on mobile)

### Data files

- [ ] Edit `data/trust/certifications.yaml` — add a new entry, rebuild, verify it appears
- [ ] Change a certification status from `planned` to `active` — badge color changes
- [ ] Add a subprocessor to `data/trust/subprocessors.yaml` — table updates
- [ ] Add a FAQ to `data/trust/faq.yaml` — accordion item appears
- [ ] Delete a data file (e.g., remove `faq.yaml`) — build still works, FAQ section hidden

### Multi-language

- [ ] Create `content/trust/security-overview.es.md` with Spanish content
- [ ] Add `[languages.es]` to `seite.toml`
- [ ] Build — verify `/es/trust/security-overview` exists with Spanish content
- [ ] Trust center index at `/es/trust/` renders in Spanish context
- [ ] Language switcher shows on trust pages when translations exist

### MCP integration

- [ ] Start MCP server with `seite mcp` on a trust-enabled project
- [ ] Send `resources/list` — verify `seite://trust` resource appears
- [ ] Send `resources/read` with `uri: "seite://trust"` — returns JSON with:
  - `config` (company, frameworks)
  - `certifications` (from YAML)
  - `subprocessors` (count + items)
  - `faq` (count + items)
  - `content_items` (trust collection items)
- [ ] Verify `seite://config` includes `trust` section
- [ ] `seite_search` with `collection: "trust"` finds trust center content

### Edge cases

- [ ] Init with `--collections trust` only (no posts/pages) — works correctly
- [ ] Init without trust collection — no trust files, no `[trust]` in seite.toml
- [ ] Build with empty trust content dir but valid data files — index renders data sections only
- [ ] Build with content but no data files — renders content sections only
- [ ] `seite new trust "Privacy Policy"` creates correct content file
- [ ] Nested trust paths work: `content/trust/certifications/soc2.md` → `/trust/certifications/soc2`

## Domain-Routed Downloads (seite.sh)

### Install script routing

- [ ] `curl -fsSL https://seite.sh/install.sh | sh` downloads and installs successfully
- [ ] `irm https://seite.sh/install.ps1 | iex` works on Windows
- [ ] `curl -fsSL https://seite.sh/install.sh | VERSION=v0.21.0 sh` pins to a specific version
- [ ] `https://seite.sh/version.txt` returns the latest release tag (e.g., `v0.2.0`)

### Download redirects

- [ ] `https://seite.sh/download/latest/seite-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the current GitHub Release asset
- [ ] `https://seite.sh/download/latest/checksums-sha256.txt` 302-redirects to the current checksums
- [ ] `https://seite.sh/download/v0.1.0/seite-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the specific version on GitHub Releases
- [ ] Checksum verification passes end-to-end (download via domain, verify sha256)

### Self-update routing

- [ ] `seite self-update --check` resolves latest version via `seite.sh/version.txt`
- [ ] `seite self-update` downloads binary through `seite.sh/download/` and installs successfully
- [ ] `seite self-update --target-version v0.1.0` downloads the specific version through the domain
- [ ] If `seite.sh` is unreachable, `self-update` falls back to GitHub API for version resolution

### Release workflow

- [ ] After a release, `seite.sh/version.txt` reflects the new tag
- [ ] After a release, `seite.sh/_redirects` maps `/download/latest/*` to the new release
- [ ] Old version URLs (`/download/v0.1.0/*`) still work after a new release
