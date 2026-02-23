---
title: Deploy History and Rollback
description: Track deploy history with content hashing, diff previews, and one-command rollback
tags:
- planned
weight: 9
---

Track deploy history and enable rollback.

- **Deploy log** — `.deploy-log.json` with timestamp, target, commit hash, build duration, and content hash
- **Deploy diff** — `seite deploy--dry-run` shows new/modified/deleted files compared to last deploy
- **Rollback** — `seite deployrollback` restores previous deploy using Netlify/Cloudflare APIs or git history
- **Atomic deploys** — skip deploy if content hash unchanged since last deploy
