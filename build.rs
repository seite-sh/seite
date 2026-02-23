//! Build script — generates releases.md from changelog entries at compile time.
//!
//! Changelog entries in `seite-sh/content/changelog/` are the single source of
//! truth. This script assembles them into `releases.md`, which gets embedded
//! into the binary via `include_str!` in `src/docs.rs`.
//!
//! This eliminates the need to keep a generated `releases.md` committed in git.
//! The shell script `scripts/generate-release-docs.sh` still exists for
//! generating the file on disk when building the docs website (seite.sh).

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let changelog_dir = Path::new("seite-sh/content/changelog");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("releases.md");

    // Rerun when changelog entries are added, removed, or modified.
    println!("cargo:rerun-if-changed=seite-sh/content/changelog");

    let mut output = String::from(
        "---\n\
         title: \"Releases\"\n\
         description: \"Release history and changelog for seite.\"\n\
         weight: 12\n\
         ---",
    );

    let files = collect_changelog_files(changelog_dir);

    if files.is_empty() {
        output.push_str("\n\nNo releases documented yet.\n");
        fs::write(&out_path, &output).unwrap();
        return;
    }

    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());

        let content = fs::read_to_string(file).expect("failed to read changelog entry");
        let title = extract_title(&content).unwrap_or_else(|| derive_title_from_filename(file));
        let body = extract_body(&content);
        let trimmed = body.trim_start_matches('\n');

        output.push_str("\n\n## ");
        output.push_str(&title);
        output.push('\n');
        output.push_str(trimmed);
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    fs::write(&out_path, &output).unwrap();
}

/// Collect `.md` files from the changelog directory, sorted reverse-alphabetically
/// by filename (newest first).
fn collect_changelog_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    files
}

/// Extract `title:` value from YAML frontmatter (between `---` delimiters).
fn extract_title(content: &str) -> Option<String> {
    let mut in_frontmatter = false;
    for line in content.lines() {
        if line.trim() == "---" {
            if in_frontmatter {
                return None;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(rest) = line.strip_prefix("title:") {
                return Some(rest.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Derive title from filename: `2026-02-20-v0-1-0.md` → `v0.1.0`.
fn derive_title_from_filename(path: &Path) -> String {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    // Strip YYYY-MM-DD- date prefix.
    let version_slug = if stem.len() > 11
        && stem.as_bytes()[4] == b'-'
        && stem.as_bytes()[7] == b'-'
        && stem.as_bytes()[10] == b'-'
    {
        &stem[11..]
    } else {
        stem
    };
    // Convert digit-hyphen-digit → digit.digit (v0-1-0 → v0.1.0).
    let chars: Vec<char> = version_slug.chars().collect();
    let mut result = String::with_capacity(version_slug.len());
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '-'
            && i > 0
            && i + 1 < chars.len()
            && chars[i - 1].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
        {
            result.push('.');
        } else {
            result.push(ch);
        }
    }
    result
}

/// Extract body: everything after the second `---` line.
fn extract_body(content: &str) -> String {
    let mut dash_count = 0;
    let mut body_lines = Vec::new();
    let mut in_body = false;

    for line in content.lines() {
        if in_body {
            body_lines.push(line);
        } else if line.trim() == "---" {
            dash_count += 1;
            if dash_count == 2 {
                in_body = true;
            }
        }
    }

    let mut body = body_lines.join("\n");
    if content.ends_with('\n') && !body_lines.is_empty() {
        body.push('\n');
    }
    body
}
