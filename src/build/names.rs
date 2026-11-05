//! How the output file system compares file names.
//!
//! The build defers writing item pages (Step 3 → Step 12) and writes
//! generated files in parallel, so it decides path collisions itself instead
//! of letting sequential writes to one file settle them. Those decisions must
//! compare paths the way the file system does: on a case-insensitive file
//! system (APFS and NTFS by default) `posts/Victim.html` and
//! `posts/victim.html` are one file, and on APFS so are the NFC and NFD
//! spellings of a name. [`NameFolding::probe`] finds out once per build;
//! [`NameFolding::key`] maps a path to the key two paths share exactly when
//! they name the same file. On a case-sensitive, non-normalizing file system
//! (the usual Linux setup) the key is the path itself.

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

/// Which name differences the output file system ignores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NameFolding {
    /// `a` and `A` name the same file.
    pub(crate) case: bool,
    /// NFC and NFD spellings name the same file.
    pub(crate) unicode: bool,
}

impl NameFolding {
    /// Probe `dir` (which must exist) with one short-lived file. If the probe
    /// cannot be written, assume exact comparison, as before deferral.
    pub(crate) fn probe(dir: &Path) -> Self {
        let probe = dir.join(".seite-name-probe-\u{e9}");
        if fs::write(&probe, b"").is_err() {
            return Self::default();
        }
        let folding = Self {
            case: dir.join(".SEITE-NAME-PROBE-\u{c9}").exists(),
            unicode: dir.join(".seite-name-probe-e\u{301}").exists(),
        };
        let _ = fs::remove_file(&probe);
        folding
    }

    /// The key two paths share exactly when they name the same file.
    pub(crate) fn key<'a>(&self, path: &'a Path) -> Cow<'a, Path> {
        if !self.case && !self.unicode {
            return Cow::Borrowed(path);
        }
        // Nearly every output path is ASCII: NFC leaves it as it is, and
        // ASCII lowercasing is all that case folding does to it.
        if let Some(text) = path.to_str().filter(|t| t.is_ascii()) {
            return if self.case {
                Cow::Owned(PathBuf::from(text.to_ascii_lowercase()))
            } else {
                Cow::Borrowed(path)
            };
        }
        let text = path.to_string_lossy();
        let text: String = if self.unicode {
            text.nfc().collect()
        } else {
            text.into_owned()
        };
        let text = if self.case { text.to_lowercase() } else { text };
        Cow::Owned(PathBuf::from(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_key_is_the_path() {
        let p = Path::new("/out/Posts/Victim.html");
        assert_eq!(NameFolding::default().key(p), p);
    }

    #[test]
    fn test_case_key_folds_case() {
        let f = NameFolding {
            case: true,
            unicode: false,
        };
        assert_eq!(
            f.key(Path::new("/out/Posts/Victim.html")),
            f.key(Path::new("/out/posts/victim.html"))
        );
    }

    #[test]
    fn test_unicode_key_folds_normalization() {
        let f = NameFolding {
            case: false,
            unicode: true,
        };
        assert_eq!(
            f.key(Path::new("/out/caf\u{e9}.html")),
            f.key(Path::new("/out/cafe\u{301}.html"))
        );
        assert_ne!(
            f.key(Path::new("/out/Cafe.html")),
            f.key(Path::new("/out/cafe.html"))
        );
    }

    #[test]
    fn test_probe_leaves_no_file_and_matches_the_file_system() {
        let dir = tempfile::tempdir().unwrap();
        let f = NameFolding::probe(dir.path());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        fs::write(dir.path().join("x.html"), "").unwrap();
        assert_eq!(f.case, dir.path().join("X.html").exists());
    }
}
