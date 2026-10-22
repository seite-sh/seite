# /seite hooks [on | off | status]

Hooks run `seite check` for the agent: after each file edit (`seite check --file`, on that file only) and when a turn ends (the full check). Only errors come back to the agent; warnings never block. If `seite` is not installed, the hooks exit silently.

- `seite hooks status`: installed or missing, per agent.
- `seite hooks on [--agents a,b]`: install the hooks.
- `seite hooks off [--agents a,b]`: remove seite's hook entries and leave every other hook alone. `seite upgrade` does not add them back.

On Windows the hook commands need a POSIX shell (Git Bash).
