//! Project-confined filesystem writes and deletes.
//!
//! `seite init`, `seite upgrade`, `seite hooks`, `seite skill` and the dev
//! server write files inside a project that may come from someone else (a
//! cloned repository). A committed symlink such as
//! `.claude/skills/seite/SKILL.md -> ~/.bashrc` must never redirect those
//! writes (or deletes) outside the project, so every such path goes through
//! [`guard`]: no component between the project root and the target may be a
//! symlink, and the canonicalized parent must stay under the canonical root.
//! The root itself may be a symlink (macOS temp dirs live under `/var`, a
//! symlink to `/private/var`).

use std::fs;
use std::io::Write;
use std::path::{Component, Path};

use anyhow::{bail, Context};

/// Refuse `target` unless it is inside `root` with no symlinked component
/// below the root, and (when its parent exists) the canonical parent is
/// inside the canonical root. Missing components are fine: they will be
/// created as real directories.
pub fn guard(root: &Path, target: &Path) -> anyhow::Result<()> {
    let rel = target.strip_prefix(root).with_context(|| {
        format!(
            "refusing to touch {}: it is outside the project at {}",
            target.display(),
            root.display()
        )
    })?;
    let mut current = root.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(part) => current.push(part),
            Component::CurDir => continue,
            _ => bail!(
                "refusing to touch {}: the path leaves the project",
                target.display()
            ),
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => bail!(
                "refusing to touch {}: {} is a symlink (seite only writes real files inside the project)",
                target.display(),
                current.display()
            ),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => {
                return Err(e).with_context(|| format!("failed to inspect {}", current.display()))
            }
        }
    }
    check_canonical_parent(root, target)
}

/// `target`, or else its deepest existing ancestor, must canonicalize to a
/// path inside the canonical root.
fn check_canonical_parent(root: &Path, target: &Path) -> anyhow::Result<()> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", root.display()))?;
    let mut ancestor: Option<&Path> = Some(target);
    while let Some(dir) = ancestor {
        if let Ok(canonical) = dir.canonicalize() {
            if !canonical.starts_with(&canonical_root) {
                bail!(
                    "refusing to touch {}: {} resolves outside the project",
                    target.display(),
                    dir.display()
                );
            }
            return Ok(());
        }
        ancestor = dir.parent();
    }
    Ok(())
}

/// [`guard`], then create `dir` (and missing parents).
pub fn create_dir_all(root: &Path, dir: &Path) -> anyhow::Result<()> {
    guard(root, dir)?;
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    guard(root, dir)
}

/// Write `content` to `path` inside `root`: guarded, parents created, and
/// replaced atomically (a temp file in the same directory, renamed over the
/// target, so an existing symlink would be replaced rather than followed).
pub fn write(root: &Path, path: &Path, content: impl AsRef<[u8]>) -> anyhow::Result<()> {
    guard(root, path)?;
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    create_dir_all(root, parent)?;
    let permissions = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Some(meta.permissions()),
        Ok(_) => bail!("refusing to write {}: not a regular file", path.display()),
        Err(_) => None,
    };
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to write {}", path.display()))?;
    tmp.write_all(content.as_ref())?;
    match permissions {
        Some(p) => tmp.as_file().set_permissions(p)?,
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tmp.as_file()
                    .set_permissions(fs::Permissions::from_mode(0o644))?;
            }
        }
    }
    tmp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Delete the file or directory at `path` inside `root` (guarded; a
/// directory is removed recursively). Missing paths are fine.
pub fn remove(root: &Path, path: &Path) -> anyhow::Result<()> {
    guard(root, path)?;
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => Err(e),
    }
    .with_context(|| format!("failed to remove {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_removes_inside_the_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let file = root.join("a/b/c.txt");
        write(root, &file, "hi").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "hi");
        write(root, &file, "again").unwrap();
        write(root, &root.join("top.txt"), "top").unwrap();
        guard(root, root).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "again");
        remove(root, &root.join("a")).unwrap();
        assert!(!root.join("a").exists());
        remove(root, &root.join("missing")).unwrap();
    }

    #[test]
    fn rejects_paths_outside_or_escaping() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("site");
        fs::create_dir_all(&root).unwrap();
        assert!(guard(&root, &tmp.path().join("other")).is_err());
        assert!(guard(&root, &root.join("../other")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_targets_and_parents() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("site");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("victim"), "keep").unwrap();

        // Symlinked file.
        std::os::unix::fs::symlink(outside.join("victim"), root.join("link")).unwrap();
        let err = write(&root, &root.join("link"), "x").unwrap_err();
        assert!(format!("{err:#}").contains("symlink"), "{err:#}");
        assert!(remove(&root, &root.join("link")).is_err());

        // Symlinked parent directory.
        std::os::unix::fs::symlink(&outside, root.join("dir")).unwrap();
        assert!(write(&root, &root.join("dir/victim"), "x").is_err());
        assert!(remove(&root, &root.join("dir")).is_err());
        assert!(create_dir_all(&root, &root.join("dir/new")).is_err());
        assert_eq!(fs::read_to_string(outside.join("victim")).unwrap(), "keep");
        assert!(!outside.join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn allows_a_symlinked_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let real = tmp.path().join("real");
        fs::create_dir_all(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write(&link, &link.join("x/y"), "ok").unwrap();
        assert_eq!(fs::read_to_string(real.join("x/y")).unwrap(), "ok");
    }
}
