# GitHub Security & Branch Protection Setup

Manual steps to complete after merging CI/CD tooling. All features below are **free for public repositories**.

## 1. Branch Protection Rules

**Settings > Branches > Add branch protection rule**

- Branch name pattern: `main`
- [x] Require a pull request before merging
  - Required approvals: 1
- [x] Require status checks to pass before merging
  - Required checks: `fmt`, `clippy`, `test`, `deny`, `doc`, `msrv`, `shellcheck`
- [x] Require branches to be up to date before merging
- [x] Require conversation resolution before merging
- [x] Do not allow bypassing the above settings

## 2. Secret Scanning + Push Protection

**Settings > Code security > Secret scanning**

- [x] Enable secret scanning
- [x] Enable push protection (blocks pushes containing detected secrets)

Both are free and zero-config for public repos.

## 3. Codecov (for your coverage PR)

1. Visit https://codecov.io and sign in with GitHub
2. Add the `seite-sh/seite` repository
3. Copy the upload token to **Settings > Secrets > Actions** as `CODECOV_TOKEN`
4. Codecov will comment on PRs with coverage diffs automatically

## 4. Fuzz Testing

Fuzz targets are set up in `fuzz/`. To run locally:

```bash
# Install cargo-fuzz (requires nightly)
cargo install cargo-fuzz

# List available targets
cargo fuzz list

# Run the shortcode parser fuzzer (runs until stopped with Ctrl+C)
cargo +nightly fuzz run fuzz_shortcode_parser

# Run with a time limit
cargo +nightly fuzz run fuzz_shortcode_parser -- -max_total_time=300
```

For continuous fuzzing, consider applying to **Google OSS-Fuzz** (free for accepted OSS projects):
https://github.com/google/oss-fuzz/blob/master/docs/getting-started/new-project-guide/rust-lang.md
