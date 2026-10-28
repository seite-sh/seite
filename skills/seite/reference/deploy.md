# /seite deploy

1. Run [check.md](check.md) first. Do not deploy a site with errors.
2. Run `seite deploy --dry-run` and show the user what it would do (target, branch, preview or production).
3. Ask the user before a real `seite deploy`: it commits, pushes, and publishes. `--preview` deploys to a staging URL; `--no-commit` skips the commit and push.
