---
title: Scheduled Publishing
description: "Hide posts with a future date from production builds, enabling schedule-ahead workflows"
tags:
- done
weight: 2
---

Shipped in v0.21.0. With `[build] schedule = true` (written by `seite init`), a post in a dated collection whose `date` is in the future is left out of every output until that day, and `expires:` takes a page down after its date. `seite serve`, `seite build --future` and `[build] future = true` include scheduled posts for preview. "Today" follows `[site] timezone`. Build, serve and check print how many posts are hidden and when the next one goes live, and generated GitHub Actions workflows rebuild daily so scheduled posts appear on time.
