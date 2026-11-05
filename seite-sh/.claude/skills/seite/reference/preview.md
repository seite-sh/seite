# /seite preview

1. `seite context` shows whether a dev server is already running, and its URL. If one is, use it. Never start a second server.
2. Otherwise start one in the background: `seite serve --no-repl` (add `--build` when `dist/` is missing). The default URL is `http://localhost:3000`; seite picks another port if that one is taken, and prints it.
3. Open the preview once, after the first visible change (template, CSS, a new page), in your built-in browser when you have one:
   - Claude desktop app: open the `seite` server from `.claude/launch.json` in its preview pane (it starts the server itself, so skip step 2).
   - Cursor: open the URL in Cursor's browser.
   - Anywhere else: run `seite serve --no-repl --open` instead of step 2, or give the user the URL.
4. The page live-reloads after every good build, and a failed build shows an error overlay with the file, line, and message. Do not reopen the page.
5. After each visible change, run `seite preview`: it prints the last build outcome (like `seite check`) and JavaScript errors reported by the open pages. Fix what it lists; `seite preview --clear-browser-errors` forgets the ones you fixed.
6. Stop the server when the user is done, if you started it.
