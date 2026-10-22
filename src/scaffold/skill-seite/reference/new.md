# /seite new <type> "<title>"

1. Look for an existing page first: `seite search <query>`.
2. Run `seite new <type> "<title>"`. `<type>` is the collection's singular name (`post`, `doc`, `page`, …). Options: `--tags a,b`, `--draft`, `--lang xx`. It prints the path of the new file, writes the collection's schema defaults, and lists required fields to "fill in".
3. Write the body in that file and set `description:`. Fill in every field `seite new` listed. `seite templates` shows each collection's fields (type, required, allowed values); custom fields go under `extra:`, built-in ones (`title`, `description`, `tags`, …) at the top level. Change frontmatter with `seite frontmatter set <path> key=value` (custom fields are stored under `extra:` for you), or edit the YAML directly.
4. To schedule a post (sites with `[build] schedule = true`), give it a future `date:`; it publishes on that date (daily rebuild). Don't link to it from published pages before then (`link-to-scheduled`). `expires: YYYY-MM-DD` takes a page down after that day. `seite serve` and `seite build --future` show scheduled posts.
5. Link to other pages with relative `.md` paths, e.g. `[intro](../docs/intro.md)`.
6. Verify as in [check.md](check.md). A `frontmatter-schema` error names the field, what it expects, and the line to fix.
