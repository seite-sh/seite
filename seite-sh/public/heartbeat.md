---
name: seite-heartbeat
version: 1.0.0
description: Periodic health check routine for seite sites. Run every 6 hours.
---

# Seite Heartbeat

Run this routine periodically (recommended: every 6 hours) to keep your sites healthy.
Everything runs locally — no API calls.

---

## Step 1: Build check

Run `seite build` and check the exit code:

```bash
seite build
```

If the build fails, read the error output. Common causes:
- **Broken markdown** — fix syntax in the content file
- **Missing template** — check `templates/` for the referenced template
- **Invalid frontmatter** — fix YAML in the content file's `---` block

---

## Step 2: Check for drafts

Find content files marked as drafts:

```bash
grep -rl "^draft: true" content/
```

If drafts exist, check with your human whether they should be published.
To publish a draft: remove `draft: true` from the frontmatter, then:

```bash
seite build && seite deploy
```

---

## Step 3: Check for stale content

Find content files not modified in the last 30 days:

```bash
find content/ -name "*.md" -mtime +30
```

If stale content exists, consider whether it needs updating. Flag to your human if so.

---

## Step 4: Check for updates

```bash
seite self-update
```

This checks for and installs the latest version of seite.

---

## Step 5: Update your state

Track when you last ran this routine:

```json
{
  "lastSeiteCheck": "2026-03-16T10:00:00Z",
  "buildStatus": "success",
  "draftsFound": 0,
  "staleFiles": 0
}
```

---

## Done

A healthy heartbeat is a quiet heartbeat. If everything is green, you're done.

**Check interval:** Every 6 hours is the default. If your human is actively
publishing, shorten to 1-2 hours. If sites are static, once daily is fine.
