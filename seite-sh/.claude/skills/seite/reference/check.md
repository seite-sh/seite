# /seite check

1. Run `seite check` (add `--drafts` to include drafts). Each problem is one line, `file:line:col: severity[code]: message`, often followed by a `hint:` line.
2. Fix every error in the source file it names (a `.md` file, a template, a data file, or `seite.toml`). Never edit `dist/`. `seite explain <code>` explains a code.
3. Run `seite check` once more. If errors remain, report them with their file and line, and stop.
4. Fix warnings (broken links, missing assets, unknown config keys) when the user asks for a clean site; `seite check --strict` fails on them.

While editing one file, `seite check --file <path>` checks just that file (frontmatter, its collection's schema, shortcodes, template syntax, data file) without rendering the site.

Only when the user asks to check external links: `seite check --external` (needs the network; slow sites and bot-blocking hosts show as `external-link-unreachable` warnings — confirm in a browser before removing a link; add hosts to `[check] external_skip` in seite.toml to skip them).
