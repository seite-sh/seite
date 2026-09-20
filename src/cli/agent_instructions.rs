//! Shared handling for cross-agent project instruction files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

pub(crate) const CLAUDE_IMPORT: &str = "@AGENTS.md";
pub(crate) const CLAUDE_SHIM: &str = "@AGENTS.md\n";

fn entry_exists(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

pub(crate) fn canonical_path(root: &Path) -> PathBuf {
    let agents_path = root.join("AGENTS.md");
    if entry_exists(&agents_path) {
        agents_path
    } else {
        root.join("CLAUDE.md")
    }
}

fn validate_write_target(path: &Path) -> anyhow::Result<Option<fs::Permissions>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                anyhow::bail!("{} is not a regular file", path.display());
            }
            let permissions = metadata.permissions();
            if permissions.readonly() {
                anyhow::bail!("{} is read-only", path.display());
            }
            Ok(Some(permissions))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn prepare_atomic_write(path: &Path, content: &str) -> anyhow::Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .context("instruction file path has no parent directory")?;
    fs::create_dir_all(parent)?;

    let existing_permissions = validate_write_target(path)?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content.as_bytes())?;
    temporary.as_file_mut().sync_all()?;
    if let Some(permissions) = existing_permissions {
        temporary.as_file().set_permissions(permissions)?;
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o644))?;
        }
    }
    Ok(temporary)
}

fn persist_atomic_write(temporary: tempfile::NamedTempFile, path: &Path) -> anyhow::Result<()> {
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let temporary = prepare_atomic_write(path, content)?;
    persist_atomic_write(temporary, path)
}

pub(crate) fn needs_migration(root: &Path) -> bool {
    let agents_path = root.join("AGENTS.md");
    let claude_path = root.join("CLAUDE.md");
    let agents_exists = entry_exists(&agents_path);
    let claude_exists = entry_exists(&claude_path);

    if (agents_exists && !agents_path.is_file()) || (claude_exists && !claude_path.is_file()) {
        return true;
    }

    match (agents_exists, claude_exists) {
        (false, false) => false,
        (false, true) | (true, false) => true,
        (true, true) => match fs::read_to_string(&claude_path) {
            Ok(content) => normalize_claude(&content) != content,
            Err(_) => true,
        },
    }
}

/// Move legacy project guidance into AGENTS.md and normalize CLAUDE.md to one
/// import plus any genuinely Claude-specific instructions.
pub(crate) fn migrate(root: &Path) -> anyhow::Result<bool> {
    let agents_path = root.join("AGENTS.md");
    let claude_path = root.join("CLAUDE.md");
    let agents_exists = entry_exists(&agents_path);
    let claude_exists = entry_exists(&claude_path);

    for (path, exists) in [(&agents_path, agents_exists), (&claude_path, claude_exists)] {
        if exists && !path.is_file() {
            anyhow::bail!("{} is not a regular file", path.display());
        }
    }

    match (agents_exists, claude_exists) {
        (false, false) => Ok(false),
        (false, true) => {
            let legacy = fs::read_to_string(&claude_path)?;
            let (import_count, without_imports) = strip_import_lines(&legacy);
            let canonical = if import_count == 0 {
                legacy
            } else {
                let trimmed = without_imports.trim_matches(['\r', '\n']);
                if trimmed.trim().is_empty() {
                    anyhow::bail!(
                        "CLAUDE.md imports AGENTS.md, but AGENTS.md is missing; restore AGENTS.md before upgrading"
                    );
                }
                format!("{trimmed}\n")
            };

            // Prepare both replacements before changing either destination so
            // validation or temporary-file failures cannot leave a partial
            // migration behind.
            let agents_write = prepare_atomic_write(&agents_path, &canonical)?;
            let claude_write = prepare_atomic_write(&claude_path, CLAUDE_SHIM)?;
            persist_atomic_write(agents_write, &agents_path)?;
            persist_atomic_write(claude_write, &claude_path)?;
            Ok(true)
        }
        (true, false) => {
            write_atomic(&claude_path, CLAUDE_SHIM)?;
            Ok(true)
        }
        (true, true) => {
            let existing = fs::read_to_string(&claude_path)?;
            let normalized = normalize_claude(&existing);
            if normalized == existing {
                return Ok(false);
            }
            write_atomic(&claude_path, &normalized)?;
            Ok(true)
        }
    }
}

fn normalize_claude(content: &str) -> String {
    let (_, without_imports) = strip_import_lines(content);
    let custom = without_imports.trim_matches(['\r', '\n']);
    if custom.trim().is_empty() {
        CLAUDE_SHIM.to_string()
    } else {
        format!("{CLAUDE_IMPORT}\n\n{custom}\n")
    }
}

fn strip_import_lines(content: &str) -> (usize, String) {
    let mut count = 0;
    let mut remainder = String::with_capacity(content.len());

    for segment in content.split_inclusive('\n') {
        let line = segment.trim_end_matches(['\r', '\n']);
        if line.trim() == CLAUDE_IMPORT {
            count += 1;
        } else {
            remainder.push_str(segment);
        }
    }

    (count, remainder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_claude_deduplicates_imports_and_preserves_custom_content() {
        let normalized =
            normalize_claude("@AGENTS.md\n\n# Claude only\nKeep this.\n  @AGENTS.md  \n");
        assert_eq!(normalized, "@AGENTS.md\n\n# Claude only\nKeep this.\n");
    }

    #[test]
    fn normalize_claude_reduces_import_only_content_to_shim() {
        assert_eq!(normalize_claude("\n@AGENTS.md\n\n"), CLAUDE_SHIM);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        write_atomic(&path, "new").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn migration_does_not_create_agents_when_claude_cannot_be_replaced() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let claude_path = tmp.path().join("CLAUDE.md");
        fs::write(&claude_path, "# Legacy instructions\n").unwrap();
        fs::set_permissions(&claude_path, fs::Permissions::from_mode(0o444)).unwrap();

        assert!(migrate(tmp.path()).is_err());
        assert!(!tmp.path().join("AGENTS.md").exists());
        assert_eq!(
            fs::read_to_string(&claude_path).unwrap(),
            "# Legacy instructions\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn migration_rejects_dangling_instruction_path_without_changing_claude() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::TempDir::new().unwrap();
        let claude_path = tmp.path().join("CLAUDE.md");
        fs::write(&claude_path, "# Legacy instructions\n").unwrap();
        symlink("missing-instructions.md", tmp.path().join("AGENTS.md")).unwrap();

        assert!(migrate(tmp.path()).is_err());
        assert!(tmp.path().join("AGENTS.md").is_symlink());
        assert_eq!(
            fs::read_to_string(&claude_path).unwrap(),
            "# Legacy instructions\n"
        );
    }
}
