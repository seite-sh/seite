# Manual Testing Checklist

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
- [ ] Banner works with all 6 bundled themes (default, minimal, dark, docs, brutalist, bento)

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
- [ ] Verify CLAUDE.md has `## Trust Center` section with correct company name

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
- [ ] `VERSION=v0.1.0 curl -fsSL https://seite.sh/install.sh | sh` pins to a specific version
- [ ] `https://seite.sh/version.txt` returns the latest release tag (e.g., `v0.2.0`)

### Download redirects

- [ ] `https://seite.sh/download/latest/page-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the current GitHub Release asset
- [ ] `https://seite.sh/download/latest/checksums-sha256.txt` 302-redirects to the current checksums
- [ ] `https://seite.sh/download/v0.1.0/page-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the specific version on GitHub Releases
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
