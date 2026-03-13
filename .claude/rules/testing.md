---
paths:
  - "tests/**"
---
# Testing

- Integration tests: `assert_cmd::Command` + `tempfile::TempDir`
- Helper: `init_site(tmp, name, title, collections)` scaffolds a site in a temp dir
- Naming: `test_{command}_{behavior}` (e.g., `test_build_excludes_drafts_by_default`)
- **Before every commit:** `cargo fmt --all && cargo clippy && cargo test` — all must pass
- CI also runs: `cargo-deny` (license audit), `cargo doc`, MSRV 1.88, `cargo-semver-checks`, ShellCheck
- Never `unwrap()` in library code — handle errors properly
