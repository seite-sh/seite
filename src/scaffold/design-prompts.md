## Design Prompts

When asked to redesign or create a theme, use one of these directions as a starting point.
Edit `templates/base.html` directly — or apply a bundled theme first with `page theme apply <name>` then edit.

**Minimal / Editorial** — Single column max 620px, Georgia serif body, geometric sans for UI elements.
No decorative elements. Bottom-border-only search input. White/off-white (`#FAF9F6`) background,
near-black (`#1A1A1A`) text, one muted link accent. Typography carries all personality.

**Bold / Neo-Brutalist** — Thick black borders (3px solid `#000000`), hard non-blurred box shadows
(`6px 6px 0 #000`). No border-radius. Saturated fill: yellow `#FFE600`, lime `#AAFF00`, or coral `#FF4D00`.
Cream (`#FFFEF0`) background. Font-weight 900. Headlines 4rem+. Buttons shift their shadow on hover to press in.

**Bento / Card Grid** — Responsive CSS grid, gap 16px, all cards border-radius 20px. Mixed card sizes
(1-, 2-, 3-col spans). Cards have independent background colors. Floating shadow:
`box-shadow: 0 4px 24px rgba(0,0,0,0.08)`. Warm neutral palette (`#F5F0EB`) with one dark-accent card per row.

**Dark / Expressive** — True black (`#000000` or `#0A0A0A`) surfaces. One neon accent:
green `#00FF87`, blue `#0066FF`, or violet `#8B5CF6`. Off-white text (`#E8E8E8`).
Translucent nav with `backdrop-filter: blur(12px)`. Visible, styled focus rings.

**Glass / Aurora** — Gradient mesh background (violet `#7B2FBE` → teal `#00C9A7`, or
indigo `#1A1040` → electric blue `#4361EE`). Floating panels: `backdrop-filter: blur(16px)`,
`rgba(255,255,255,0.10)` fill, `1px solid rgba(255,255,255,0.2)` border. Use for cards/nav only.

**Accessible / High-Contrast** — WCAG AAA ratios. Min 16px body. 3px colored focus rings
(design feature, not afterthought). Min 44px click targets. One semantic accent. No color-only
information. Full `prefers-reduced-motion: reduce` support.

