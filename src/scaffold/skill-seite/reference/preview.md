# /seite preview

1. `seite context` shows whether a dev server is already running, and its URL. If one is, use it. Never start a second server.
2. Otherwise start one in the background: `seite serve --no-repl` (add `--build` when `dist/` is missing). The default URL is `http://localhost:3000`; seite picks another port if that one is taken, and prints it.
3. Open the preview once, after the first visible change (template, CSS, a new page):
   - Claude desktop app: open the `seite` server from `.claude/launch.json` in its preview pane (it starts the server itself, so skip step 2).
   - Cursor: open the URL in Cursor's browser.
   - Anywhere else: run `seite serve --no-repl --open` instead of step 2, or give the user the URL.
4. Live reload refreshes the page after later edits. Do not reopen it.
5. Stop the server when the user is done, if you started it.
