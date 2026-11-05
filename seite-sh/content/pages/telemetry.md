---
title: "Telemetry"
description: "What anonymous usage data the seite CLI and installer send, when they send it, and how to turn it off with seite telemetry off or DO_NOT_TRACK=1."
---

The `seite` CLI and the install scripts send a small, anonymous usage event so we can see which commands people use, on which platforms, and how often they fail. This page lists everything that is sent. It is the whole list: the payloads are built in [`src/telemetry.rs`](https://github.com/seite-sh/seite/blob/main/src/telemetry.rs), [`install.sh`](https://github.com/seite-sh/seite/blob/main/install.sh) and [`install.ps1`](https://github.com/seite-sh/seite/blob/main/seite-sh/public/install.ps1).

## What the CLI sends

After a command finishes, `seite` sends one event with:

| Field | Example | Meaning |
|---|---|---|
| command | `build` | The top-level command name only. No arguments, flags, file names or paths. |
| `version` | `0.21.0` | The seite version. |
| `os`, `arch` | `macos`, `aarch64` | Operating system and CPU architecture. |
| `success` | `true` | Whether the command succeeded. |
| `duration_bucket` | `<1s` | A coarse bucket (`<1s`, `1-5s`, `5-30s`, `30s+`), never the exact time. |

That is all. No site content, titles, URLs, paths, config values, error messages, user names, machine IDs or install IDs are sent. There is no identifier that links one event to another. Like any HTTPS request, the event reaches the server from your network's IP address.

The event is a JSON `POST` to `https://who.seite.sh/api/event` with the user agent `seite-cli/<version>`. It is sent from a background thread with a 2-second timeout: it never blocks the command, never changes its output or exit code, and a failed send is ignored.

No event is sent for `seite self-update`, `seite mcp`, `seite perf`, `seite completions`, `seite telemetry`, or the check hooks (`seite check --hook …`).

## What the installer sends

`install.sh` and `install.ps1` send one `install` event after a successful install, with the installed version, the operating system, the architecture, and which script ran (`install.sh` or `install.ps1`). It runs in the background with a 2-second timeout and never fails the install.

## Turning it off

Any of these turns telemetry off:

```bash
seite telemetry off          # saved in ~/.seite/telemetry.json
export DO_NOT_TRACK=1        # any non-empty value; also skips the installer event
export SEITE_TELEMETRY=0     # or off, false, no; also skips the installer event
```

`seite telemetry on` turns it back on, and `seite telemetry status` shows whether it is on and why. The CLI decides in this order: `DO_NOT_TRACK`, then `SEITE_TELEMETRY`, then a `CI` environment variable (CI runs send nothing unless `SEITE_TELEMETRY=1`), then your saved preference. With none of them set, telemetry is on.

The first time `seite` sends an event from an interactive terminal, it prints a one-time notice that links to this page.
