# /seite new <type> "<title>"

1. Look for an existing page first: `seite search <query>`.
2. Run `seite new <type> "<title>"`. `<type>` is the collection's singular name (`post`, `doc`, `page`, …). Options: `--tags a,b`, `--draft`, `--lang xx`. It prints the path of the new file.
3. Write the body in that file and set `description:`. Change frontmatter with `seite frontmatter set <path> key=value`, or edit the YAML directly.
4. Link to other pages with relative `.md` paths, e.g. `[intro](../docs/intro.md)`.
5. Verify as in [check.md](check.md).
