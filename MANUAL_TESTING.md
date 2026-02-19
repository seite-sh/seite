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
