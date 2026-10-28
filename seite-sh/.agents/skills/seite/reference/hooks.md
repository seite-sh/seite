# /seite hooks [on | off | status]

Hooks run `seite check` for the agent: after each file edit (`seite check --file`, on that file only) and when a turn ends (the full check). Only errors come back to the agent; warnings never block. If `seite` is not installed, the hooks exit silently.

- `seite hooks status`: installed or missing, per agent.
- `seite hooks on [--agents a,b] [--tier stop|post-edit|all]`: install the hooks (default: both tiers).
- `seite hooks off [--agents a,b] [--tier stop|post-edit|all]`: remove seite's hook entries and leave every other hook alone. `seite upgrade` does not add them back.

On Windows the hook commands need a POSIX shell (Git Bash).

Hooks that came with the seite Claude Code plugin (`/plugin install seite@seite`) are not in the project files: `seite hooks` neither lists nor removes them. Turn them off by disabling the plugin in `/plugin` (or `claude plugin disable seite@seite`). The plugin's end-of-turn check runs only after an edit inside the site, and the plugin's hooks stay quiet when the project has its own copy.
