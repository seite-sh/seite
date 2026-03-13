# Publish Draft — seite Edition

Publish a draft article from `content/` by removing the `draft: true` flag, building the site, and optionally deploying.

## Usage

`/publish-draft <file-path>`

Example: `/publish-draft content/posts/2026-03-12-rust-error-handling.md`

## Workflow

1. **Read the specified file** and verify it exists and has `draft: true` in frontmatter
2. **Run the content scrubber** — check for AI watermarks and telltale patterns (same as SEOMachine's `/scrub`). Fix any issues found
3. **Remove `draft: true`** from the YAML frontmatter (or set it to `draft: false`)
4. **Run `seite build`** to verify the site builds successfully with the new content
5. **Report the result** — show the article title, URL it will be published at, and word count
6. **Ask if user wants to deploy** — if yes, run `seite deploy`

## Important

- Do NOT use WordPress or any external CMS — this is a seite static site
- The article stays in its current location in `content/` — seite serves it from there
- After publishing, the article will appear at its URL (e.g., `/posts/rust-error-handling`)
- Run `seite serve` if the user wants to preview before deploying
