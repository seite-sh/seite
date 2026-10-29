---
title: Deploy History and Rollback
description: Track deploy history with content hashing, diff previews, and one-command rollback
tags:
- planned
weight: 9
---

Track deploy history and enable rollback.

- **Deploy log**: `.deploy-log.json` with timestamp, target, commit hash, build duration, and content hash
- **Deploy diff**: extend the existing `seite deploy --dry-run` (which already reports the deploy plan) to list new/modified/deleted files compared to the last deploy
- **Rollback**: `seite deploy rollback` restores previous deploy using Netlify/Cloudflare APIs or git history
- **Atomic deploys**: skip deploy if content hash unchanged since last deploy
