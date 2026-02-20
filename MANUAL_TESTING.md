# Manual Testing Checklist

Tests that require a real browser or deployed environment and can't be covered by `cargo test`.

## Analytics & Cookie Consent Banner

### Google Analytics 4 (direct)

- [ ] Add `[analytics]` with `provider = "google"` and a real `G-XXXXXXX` ID to `page.toml`
- [ ] Run `page build && page serve`, open in browser
- [ ] Verify GA4 script tag appears in page source `<head>`
- [ ] Verify real-time traffic shows up in Google Analytics dashboard
- [ ] Verify no cookie consent banner is shown

### Google Analytics 4 (with consent)

- [ ] Set `cookie_consent = true` in `[analytics]`
- [ ] Run `page build && page serve`, open in browser
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
- [ ] Multiple `page build` runs — analytics injected exactly once (not duplicated)
- [ ] Works with `page serve` live reload (rebuilt pages still have analytics)
- [ ] Works in workspace mode (`page build --site blog`)

## Domain-Routed Downloads (pagecli.dev)

### Install script routing

- [ ] `curl -fsSL https://pagecli.dev/install.sh | sh` downloads and installs successfully
- [ ] `irm https://pagecli.dev/install.ps1 | iex` works on Windows
- [ ] `VERSION=v0.1.0 curl -fsSL https://pagecli.dev/install.sh | sh` pins to a specific version
- [ ] `https://pagecli.dev/version.txt` returns the latest release tag (e.g., `v0.2.0`)

### Download redirects

- [ ] `https://pagecli.dev/download/latest/page-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the current GitHub Release asset
- [ ] `https://pagecli.dev/download/latest/checksums-sha256.txt` 302-redirects to the current checksums
- [ ] `https://pagecli.dev/download/v0.1.0/page-x86_64-unknown-linux-gnu.tar.gz` 302-redirects to the specific version on GitHub Releases
- [ ] Checksum verification passes end-to-end (download via domain, verify sha256)

### Self-update routing

- [ ] `page self-update --check` resolves latest version via `pagecli.dev/version.txt`
- [ ] `page self-update` downloads binary through `pagecli.dev/download/` and installs successfully
- [ ] `page self-update --target-version v0.1.0` downloads the specific version through the domain
- [ ] If `pagecli.dev` is unreachable, `self-update` falls back to GitHub API for version resolution

### Release workflow

- [ ] After a release, `pagecli.dev/version.txt` reflects the new tag
- [ ] After a release, `pagecli.dev/_redirects` maps `/download/latest/*` to the new release
- [ ] Old version URLs (`/download/v0.1.0/*`) still work after a new release
