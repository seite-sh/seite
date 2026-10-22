# /seite upgrade

Brings this project's agent files and config up to date with the installed `seite`.

1. Run `seite upgrade --check`. It lists pending changes and exits 1 when there are some.
2. Show the list to the user. After they agree, run `seite upgrade --force`.
3. `--agents claude,codex,…` changes which coding agents the project is set up for; `--mcp` also adds the seite MCP server to their configs.
4. Verify as in [check.md](check.md).

If the installed `seite` is older than the project expects, tell the user to run `seite self-update`.
