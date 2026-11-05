# /seite audit

1. Run `seite audit --json` (add `--category seo|geo|a11y|content|perf` to focus). `data.summary` has the score and counts; each entry of `data.findings` has a `code`, `severity`, the `page_url`, the `file` and `line` to edit, a `message` and a `hint`.
2. Fix errors first, then warnings, in the file each finding names: content `.md` files (title, description, word count, alt text, links), then templates (a finding that names a template affects every page using it — fix it once). Never edit `dist/`. `seite explain <code>` explains a code.
3. Don't invent content to pass a check: for a thin page or a missing description, write real text from what the page is about, or ask the user. Notices are suggestions; fix them only when the user asks for a polished site.
4. Run `seite check`, then `seite audit` once more. Report the new score and the findings that remain (with file and line), and stop. Don't loop.

For CI, `seite audit --fail-on error` exits 1 on any error-level finding.
