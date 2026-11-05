# /seite new <type> "<title>"

1. Look for an existing page first: `seite search <query>`.
2. Run `seite new <type> "<title>"`. `<type>` is the collection's singular name (`post`, `doc`, `page`, …). Options: `--tags a,b`, `--draft`, `--lang xx`. It prints the path of the new file, writes the collection's schema defaults, and lists required fields to "fill in".
3. Write the body in that file and set `description:`. Fill in every field `seite new` listed. `seite templates` shows each collection's fields (type, required, allowed values); custom fields may go at the top level or under `extra:` (not both), built-in ones (`title`, `description`, `tags`, …) at the top level. Change frontmatter with `seite frontmatter set <path> key=value` (an existing key keeps its place; new custom fields are stored under `extra:`), or edit the YAML directly.
4. Group it: `tags: [a, b]`; for a `[[taxonomies]]` entry in `seite.toml` (e.g. `categories`), a top-level key of that name: `categories: [Rust]`. Multi-part articles share `series: "Name"` (optional `series_order: 2`); related pages and the series box come from these, nothing else to write.
5. To schedule a post (sites with `[build] schedule = true`), give it a future `date:`; it publishes on that date (daily rebuild). Don't link to it from published pages before then (`link-to-scheduled`). `expires: YYYY-MM-DD` takes a page down after that day. `seite serve` and `seite build --future` show scheduled posts.
6. Link to other pages with relative `.md` paths, e.g. `[intro](../docs/intro.md)`.
7. Verify as in [check.md](check.md). A `frontmatter-schema` error names the field, what it expects, and the line to fix.
8. Moving a whole site in? `seite import <wordpress|ghost|jekyll|hugo> <export-or-dir>` (`--dry-run` first), then work through `.seite/cache/import-report.md` and verify as in [check.md](check.md).
