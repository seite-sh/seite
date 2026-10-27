# /seite upgrade

Brings this project's agent files and config up to date with the installed `seite`.

1. Run `seite upgrade --check`. It lists pending changes and exits 1 when there are some.
2. Show the list to the user. After they agree, run `seite upgrade --agent --force`. It applies the upgrade, then prints a migration brief: what changed automatically, the release notes since this project's recorded version, and what to do next.
3. Follow the brief: apply any manual migration steps the release notes describe to this site's content, templates, and config, keep the site's own customizations, and re-run `seite check` and `seite build` until both are clean. Report what you changed.
4. `--agents claude,codex,…` changes which coding agents the project is set up for; `--mcp` also adds the seite MCP server to their configs.

If the installed `seite` is older than the project expects, tell the user to run `seite self-update`.
