---
name: seite-heartbeat
version: 1.1.0
description: Periodic health check routine for seite sites (seite 0.21+). Read-only; run every 6 hours.
---

# Seite Heartbeat

Run this routine periodically (recommended: every 6 hours) to keep your sites healthy.
Everything runs locally and is read-only: no API calls, no writes to `dist/`, nothing
deployed or installed without your human.

---

## Step 1: Check the site

```bash
seite check
```

`seite check` validates config, content, templates and internal links in a scratch build
(`dist/` is untouched). It exits non-zero when there are errors and prints each one as
`file:line:col` with a stable code; `seite explain <code>` shows the fix. Use `--json` to
parse the result. If there are errors, fix them (or flag them to your human) before anything else.

---

## Step 2: Audit the built site

```bash
seite audit --min-severity warning
```

`seite audit` reports SEO, GEO (AI visibility), accessibility, content and performance
findings with a score, also from a scratch build. Fix what you can; flag the rest.

---

## Step 3: Content health: drafts, gaps, scheduled posts

```bash
seite stats
```

`seite stats` counts items and drafts per collection, plus items missing a description or
tags, future-dated (scheduled) and expired posts, and missing translations.

If drafts exist, check with your human whether they should be published. To publish one,
remove `draft: true` from its frontmatter, run `seite check`, and deploy only with your
human's go-ahead (`seite deploy`).

---

## Step 4: Check for stale content

Find content files not modified in the last 30 days:

```bash
find content/ -name "*.md" -mtime +30
```

If stale content exists, consider whether it needs updating. Flag to your human if so.

---

## Step 5: Check for updates (don't install)

```bash
seite self-update --check
```

This only reports whether a newer seite exists. Never install it unattended: tell your
human, and let them run `seite self-update` and then `seite upgrade` (which updates the
project's agent files and shows what changed).

---

## Step 6: Update your state

Track when you last ran this routine:

```json
{
  "lastSeiteCheck": "2026-10-29T10:00:00Z",
  "checkErrors": 0,
  "auditScore": 100,
  "draftsFound": 0,
  "staleFiles": 0,
  "updateAvailable": false
}
```

---

## Done

A healthy heartbeat is a quiet heartbeat. If everything is green, you're done.

**Check interval:** Every 6 hours is the default. If your human is actively
publishing, shorten to 1-2 hours. If sites are static, once daily is fine.
