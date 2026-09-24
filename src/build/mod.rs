pub mod access;
pub mod analytics;
pub mod base_path;
pub mod cache;
pub mod code_copy;
pub mod discovery;
pub mod feed;
pub mod images;
pub mod links;
pub mod markdown;
pub mod math;
pub mod mermaid;
pub mod redirects;
pub mod sitemap;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use crate::config::{AnalyticsSection, CollectionConfig, ResolvedPaths, SiteConfig};
use crate::content::{self, ContentItem, Frontmatter};
use crate::error::{PageError, Result};
use crate::output::CommandOutput;
use crate::templates;

pub struct BuildOptions {
    pub include_drafts: bool,
    /// Enable incremental builds: only rebuild changed content items.
    /// Templates, data, and config changes still trigger full rebuilds.
    pub incremental: bool,
}

pub struct BuildResult {
    pub collections: HashMap<String, Vec<ContentItem>>,
    pub stats: BuildStats,
    pub link_check: links::LinkCheckResult,
    /// Per-subdomain build results.
    pub subdomain_builds: Vec<SubdomainBuildInfo>,
    /// Non-fatal problems that did not stop the build but likely produced
    /// unexpected output (e.g. user templates that failed to parse and were
    /// replaced by the built-in defaults). Also printed to stderr.
    pub warnings: Vec<String>,
}

/// Build info for a single subdomain collection.
pub struct SubdomainBuildInfo {
    pub collection_name: String,
    pub subdomain: String,
    pub output_dir: PathBuf,
    pub base_url: String,
    pub stats: BuildStats,
}

#[derive(Debug, Serialize)]
pub struct BuildStats {
    pub items_built: HashMap<String, usize>,
    pub static_files_copied: usize,
    pub public_files_copied: usize,
    pub data_files_loaded: usize,
    pub duration_ms: u64,
    /// Per-step timing: (step_name, duration_ms)
    pub step_timings: Vec<(String, f64)>,
    /// Whether this was an incremental build.
    pub incremental: bool,
    /// Number of content items skipped (unchanged).
    pub items_skipped: usize,
}

impl CommandOutput for BuildStats {
    fn human_display(&self) -> String {
        let mut sorted_items: Vec<(&String, &usize)> = self.items_built.iter().collect();
        sorted_items.sort_by_key(|(a, _)| *a);
        let parts: Vec<String> = sorted_items
            .iter()
            .map(|(name, count)| format!("{count} {name}"))
            .collect();
        let build_type = if self.incremental {
            "Incremental build"
        } else {
            "Built"
        };
        let mut out = format!(
            "{build_type} {} in {:.1}s ({} static files copied",
            parts.join(", "),
            self.duration_ms as f64 / 1000.0,
            self.static_files_copied
        );
        if self.public_files_copied > 0 {
            out.push_str(&format!(
                ", {} public files copied",
                self.public_files_copied
            ));
        }
        if self.data_files_loaded > 0 {
            out.push_str(&format!(", {} data files loaded", self.data_files_loaded));
        }
        if self.incremental && self.items_skipped > 0 {
            out.push_str(&format!(", {} items unchanged", self.items_skipped));
        }
        out.push(')');
        out
    }
}

impl BuildStats {
    /// Per-step timing breakdown for `--verbose` output. `None` when no step
    /// timings were recorded.
    pub fn timings_display(&self) -> Option<String> {
        if self.step_timings.is_empty() {
            return None;
        }
        let mut out = String::from("  Timings:");
        for (name, ms) in &self.step_timings {
            if *ms >= 1.0 {
                out.push_str(&format!("\n    {name}: {ms:.1}ms"));
            } else {
                out.push_str(&format!("\n    {name}: <1ms"));
            }
        }
        Some(out)
    }
}

#[derive(Serialize)]
struct SiteContext {
    title: String,
    description: String,
    base_url: String,
    /// URL path prefix extracted from `base_url` for subpath deployments
    /// (e.g., `"/repo"` for GitHub Pages project sites). Empty for root deployments.
    base_path: String,
    language: String,
    author: String,
}

#[derive(Serialize)]
struct PageContext {
    title: String,
    content: String,
    date: Option<String>,
    /// Last-modified date string (ISO 8601). Populated from `updated` frontmatter field.
    updated: Option<String>,
    description: Option<String>,
    /// Absolute URL or path to social-preview image (og:image / twitter:image).
    image: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
    /// Collection name ("posts", "docs", "pages", etc.).
    collection: String,
    /// Per-page robots meta value, e.g. "noindex".
    robots: Option<String>,
    /// Word count of the raw markdown body.
    word_count: usize,
    /// Estimated reading time in minutes (based on 238 WPM average).
    reading_time: usize,
    /// Auto-extracted excerpt rendered as HTML.
    excerpt: String,
    /// Table of contents entries extracted from heading hierarchy.
    toc: Vec<markdown::TocEntry>,
    /// Arbitrary key-value data from frontmatter `extra` field.
    extra: std::collections::HashMap<String, serde_yaml_ng::Value>,
}

/// Lightweight previous/next post link exposed to templates as `prev_post` / `next_post`.
#[derive(Serialize, Clone)]
struct AdjacentPost {
    title: String,
    url: String,
    date: Option<String>,
    description: Option<String>,
}

#[derive(Serialize)]
struct CollectionContext {
    name: String,
    label: String,
    items: Vec<ItemSummary>,
}

#[derive(Serialize, Clone)]
struct ItemSummary {
    title: String,
    date: Option<String>,
    description: Option<String>,
    slug: String,
    tags: Vec<String>,
    url: String,
    /// Word count of the raw markdown body.
    word_count: usize,
    /// Estimated reading time in minutes.
    reading_time: usize,
    /// Auto-extracted excerpt rendered as HTML.
    excerpt: String,
}

#[derive(Serialize, Clone)]
struct NavItem {
    title: String,
    url: String,
    active: bool,
}

#[derive(Serialize, Clone)]
struct NavSection {
    name: String,
    label: String,
    items: Vec<NavItem>,
}

#[derive(Serialize)]
struct PaginationContext {
    current_page: usize,
    total_pages: usize,
    prev_url: Option<String>,
    next_url: Option<String>,
    base_url: String,
}

/// A translation link used in templates and the sitemap's xhtml:link alternates.
#[derive(Serialize, Clone)]
pub(crate) struct TranslationLink {
    pub lang: String,
    pub url: String,
}

#[derive(Serialize)]
struct SearchEntry<'a> {
    title: &'a str,
    description: Option<&'a str>,
    url: &'a str,
    collection: &'a str,
    tags: &'a [String],
    date: Option<String>,
    lang: &'a str,
}

impl SiteContext {
    fn from_config(config: &SiteConfig) -> Self {
        Self {
            title: config.site.title.clone(),
            description: config.site.description.clone(),
            base_url: config.site.base_url.clone(),
            base_path: config.base_path(),
            language: config.site.language.clone(),
            author: config.site.author.clone(),
        }
    }

    fn for_lang(config: &SiteConfig, lang: &str) -> Self {
        Self {
            title: config.title_for_lang(lang).to_string(),
            description: config.description_for_lang(lang).to_string(),
            base_url: config.site.base_url.clone(),
            base_path: config.base_path(),
            language: lang.to_string(),
            author: config.site.author.clone(),
        }
    }
}

pub fn build_site(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    opts: &BuildOptions,
) -> Result<BuildResult> {
    // Subdomain collections are removed from the main output and rebuilt as
    // independent root-mounted sites.
    let has_subdomains = config.has_subdomains();
    let main_config;
    let effective_config = if has_subdomains {
        main_config = {
            let mut c = config.clone();
            c.collections = config.main_site_collections();
            c
        };
        &main_config
    } else {
        config
    };

    // Incremental build: check what changed and potentially skip the build entirely
    let config_path = paths.root.join("seite.toml");
    let mut changeset = None;
    if opts.incremental {
        let build_cache = cache::BuildCache::load(&paths.root);
        let cs = build_cache.diff(
            &config_path,
            &paths.content,
            &paths.templates,
            &paths.data_dir,
            &paths.static_dir,
        );
        let all_outputs_exist = paths.output.exists()
            && config
                .subdomain_collections()
                .iter()
                .all(|collection| paths.subdomain_output(&collection.name).exists());
        if cs.is_empty() && all_outputs_exist {
            // Even a content no-op must refresh generated access artifacts so
            // upgrading seite applies security migrations to retained output.
            refresh_access_artifacts(effective_config, paths)?;
            refresh_subdomain_access_artifacts(config, paths)?;
            return Ok(BuildResult {
                collections: HashMap::new(),
                stats: BuildStats {
                    items_built: HashMap::new(),
                    static_files_copied: 0,
                    public_files_copied: 0,
                    data_files_loaded: 0,
                    duration_ms: 0,
                    step_timings: Vec::new(),
                    incremental: true,
                    items_skipped: cs.total_content,
                },
                link_check: links::LinkCheckResult::default(),
                subdomain_builds: Vec::new(),
                warnings: Vec::new(),
            });
        }
        if let Some(ref reason) = cs.full_rebuild_reason {
            tracing::debug!("Full rebuild required: {reason}");
        } else {
            tracing::debug!(
                "Incremental build: {} changed, {} deleted",
                cs.changed_content.len(),
                cs.deleted_content.len()
            );
        }
        changeset = Some(cs);
    }

    // Full builds write into a staging directory next to the output and are
    // swapped into place only after every step (including subdomain builds)
    // succeeded, so a failed build leaves the previous output untouched.
    // Incremental builds (dev server: only content changed, nothing deleted)
    // update the existing output in place. They never clean it, so a failure
    // there can leave a mix of old and freshly rendered pages but never wipes
    // the site; the next successful build brings it back in sync.
    clean_stale_build_dirs(&paths.output);
    let in_place = changeset
        .as_ref()
        .is_some_and(|cs| !cs.needs_full_rebuild && cs.deleted_content.is_empty())
        && paths.output.exists();
    let mut staged = StagedOutputs::default();
    let mut main_paths = paths.clone();
    if !in_place {
        main_paths.output = staged.stage(&paths.root, &paths.output)?;
    }

    // Compute the subdomain rewrite map from the ORIGINAL config (before filtering),
    // since the main-site config has subdomain collections removed.
    let main_rewrites = if has_subdomains {
        config.subdomain_rewrite_map()
    } else {
        HashMap::new()
    };
    let rewrites_ref = if main_rewrites.is_empty() {
        None
    } else {
        Some(&main_rewrites)
    };

    let result = build_site_inner(
        effective_config,
        &main_paths,
        opts,
        rewrites_ref,
        &changeset,
        &paths.output,
    )?;

    let mut warnings = result.warnings;

    // Build subdomain collections into their own output directories
    let subdomain_builds = if has_subdomains {
        build_subdomain_sites(config, paths, opts, &mut warnings, &mut staged)?
    } else {
        Vec::new()
    };

    // Everything built: swap staged outputs into place, then drop outputs of
    // collections that are no longer deployed to a subdomain.
    staged.commit()?;
    if has_subdomains {
        prune_stale_subdomain_outputs(config, paths);
    }

    // Save build cache after successful build
    if opts.incremental {
        let new_cache = cache::BuildCache::snapshot(
            &config_path,
            &paths.content,
            &paths.templates,
            &paths.data_dir,
            &paths.static_dir,
        );
        if let Err(e) = new_cache.save(&paths.root) {
            tracing::warn!("Failed to save build cache: {e}");
        }
    }

    Ok(BuildResult {
        collections: result.collections,
        stats: result.stats,
        link_check: result.link_check,
        subdomain_builds,
        warnings,
    })
}

/// Suffix (plus process id) of the directory a full build writes into.
const STAGING_MARKER: &str = ".staging-";
/// Suffix (plus process id) the previous output is renamed to during the swap.
const OLD_OUTPUT_MARKER: &str = ".old-";

/// True for `{output_name}.staging-<pid>` / `{output_name}.old-<pid>`.
fn is_scratch_output_name(name: &str, output_name: &str) -> bool {
    let Some(rest) = name.strip_prefix(output_name) else {
        return false;
    };
    let Some(pid) = rest
        .strip_prefix(STAGING_MARKER)
        .or_else(|| rest.strip_prefix(OLD_OUTPUT_MARKER))
    else {
        return false;
    };
    !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit())
}

/// Sibling of `output` named `{name}{marker}{pid}`. `None` when `output` has
/// no parent/name, or is the site root or one of its ancestors (a
/// misconfiguration we don't try to stage around).
fn scratch_dir_for(root: &Path, output: &Path, marker: &str) -> Option<PathBuf> {
    if root.starts_with(output) {
        return None;
    }
    let name = output.file_name()?.to_str()?;
    let parent = output.parent()?;
    Some(parent.join(format!("{name}{marker}{}", std::process::id())))
}

/// Remove staging/old directories left next to `output` by a build that
/// crashed or was killed. Best-effort: failures are only logged.
///
/// Directories owned by a build that is still running (e.g. the dev server
/// rebuilding while an agent runs `seite build`) are left alone.
fn clean_stale_build_dirs(output: &Path) {
    let (Some(parent), Some(name)) = (output.parent(), output.file_name()) else {
        return;
    };
    let Some(name) = name.to_str() else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_name = entry.file_name();
        let Some(entry_name) = entry_name.to_str() else {
            continue;
        };
        if is_scratch_output_name(entry_name, name)
            && entry.path().is_dir()
            && !scratch_owner_running(entry_name, &entry.path())
        {
            if let Err(e) = fs::remove_dir_all(entry.path()) {
                tracing::debug!(
                    "Failed to remove stale build dir {}: {e}",
                    entry.path().display()
                );
            }
        }
    }
}

/// Whether the build that created scratch dir `name` (`…staging-<pid>` /
/// `…old-<pid>`) may still be running.
fn scratch_owner_running(name: &str, path: &Path) -> bool {
    let Some(pid) = name
        .rsplit(['-'])
        .next()
        .and_then(|p| p.parse::<u32>().ok())
    else {
        return false;
    };
    if pid == std::process::id() {
        return true;
    }
    process_running(pid, path)
}

#[cfg(unix)]
fn process_running(pid: u32, _path: &Path) -> bool {
    let Some(pid) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    // Signal 0 only checks existence; EPERM means it exists but isn't ours.
    match rustix::process::test_kill_process(pid) {
        Ok(()) => true,
        Err(e) => e == rustix::io::Errno::PERM,
    }
}

/// Without a cheap portable liveness check, treat recently touched scratch
/// dirs as belonging to a running build.
#[cfg(not(unix))]
fn process_running(_pid: u32, path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < std::time::Duration::from_secs(3600))
}

/// True when `path` is inside a build output directory (the main output,
/// `dist-subdomains/`, or a staging/old sibling of the output). The dev
/// server's file watcher ignores these so builds don't trigger rebuilds.
pub fn is_build_output_path(paths: &ResolvedPaths, path: &Path) -> bool {
    let subdomains_root = paths.root.join("dist-subdomains");
    if path.starts_with(&paths.output) || path.starts_with(&subdomains_root) {
        return true;
    }
    let (Some(parent), Some(name)) = (
        paths.output.parent(),
        paths.output.file_name().and_then(|n| n.to_str()),
    ) else {
        return false;
    };
    path.strip_prefix(parent)
        .ok()
        .and_then(|rest| rest.components().next())
        .and_then(|first| first.as_os_str().to_str())
        .is_some_and(|first| is_scratch_output_name(first, name))
}

/// Output directories being built in staging, swapped into place by
/// [`StagedOutputs::commit`]. Staging directories that were never committed
/// (the build failed) are removed on drop.
#[derive(Default)]
struct StagedOutputs {
    /// (staging dir, final output dir)
    dirs: Vec<(PathBuf, PathBuf)>,
}

impl StagedOutputs {
    /// Start staging `output`; returns the directory to build into. Falls
    /// back to building in place when the output can't be staged safely.
    fn stage(&mut self, root: &Path, output: &Path) -> Result<PathBuf> {
        let Some(staging) = scratch_dir_for(root, output, STAGING_MARKER) else {
            return Ok(output.to_path_buf());
        };
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        self.dirs.push((staging.clone(), output.to_path_buf()));
        Ok(staging)
    }

    /// Swap every staged directory into place.
    fn commit(mut self) -> Result<()> {
        while !self.dirs.is_empty() {
            let (staging, output) = self.dirs.remove(0);
            let root = output.parent().unwrap_or(Path::new("")).to_path_buf();
            if let Err(e) = swap_into_place(&root, &staging, &output) {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
        }
        Ok(())
    }
}

impl Drop for StagedOutputs {
    fn drop(&mut self) {
        for (staging, _) in &self.dirs {
            if staging.exists() {
                if let Err(e) = fs::remove_dir_all(staging) {
                    tracing::debug!("Failed to remove staging dir {}: {e}", staging.display());
                }
            }
        }
    }
}

/// Replace `output` with `staging`.
///
/// Renames the previous output aside, renames staging into place, then
/// deletes the old copy. Two renames rather than one because renaming onto an
/// existing directory fails on Windows (and on Unix when it's non-empty). If
/// the second rename fails the previous output is restored. If the output
/// itself can't be renamed (a mount point, a symlink, or open handles on
/// Windows), its contents are replaced instead.
fn swap_into_place(parent: &Path, staging: &Path, output: &Path) -> Result<()> {
    let is_symlink = fs::symlink_metadata(output)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if !output.exists() && !is_symlink {
        fs::create_dir_all(parent)?;
        fs::rename(staging, output)?;
        return Ok(());
    }

    if !is_symlink {
        if let Some(old) = scratch_dir_for(parent, output, OLD_OUTPUT_MARKER) {
            if old.exists() {
                fs::remove_dir_all(&old)?;
            }
            match fs::rename(output, &old) {
                Ok(()) => {
                    if let Err(e) = fs::rename(staging, output) {
                        // Put the previous output back so nothing is lost.
                        let _ = fs::rename(&old, output);
                        return Err(e.into());
                    }
                    if let Err(e) = fs::remove_dir_all(&old) {
                        // Harmless: the next build removes leftovers.
                        tracing::debug!("Failed to remove {}: {e}", old.display());
                    }
                    return Ok(());
                }
                Err(e) => {
                    tracing::debug!(
                        "Could not move {} aside ({e}); replacing its contents instead",
                        output.display()
                    );
                }
            }
        }
    }

    replace_dir_contents(staging, output)?;
    fs::remove_dir_all(staging)?;
    Ok(())
}

/// Empty `output` and move (or, across filesystems, copy) everything from
/// `staging` into it.
fn replace_dir_contents(staging: &Path, output: &Path) -> Result<()> {
    for entry in fs::read_dir(output)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let dest = output.join(entry.file_name());
        if fs::rename(entry.path(), &dest).is_err() {
            copy_recursively(&entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn copy_recursively(src: &Path, dest: &Path) -> Result<()> {
    if src.is_file() {
        fs::copy(src, dest)?;
        return Ok(());
    }
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Remove `dist-subdomains/<name>` directories for collections that are no
/// longer deployed to a subdomain (previously handled by wiping the whole
/// directory before every build). Best-effort.
fn prune_stale_subdomain_outputs(config: &SiteConfig, paths: &ResolvedPaths) {
    let subdomains_root = paths.root.join("dist-subdomains");
    let Ok(entries) = fs::read_dir(&subdomains_root) else {
        return;
    };
    let current: HashSet<String> = config
        .subdomain_collections()
        .iter()
        .map(|c| c.name.clone())
        .collect();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !current.contains(&name) && entry.path().is_dir() {
            if let Err(e) = fs::remove_dir_all(entry.path()) {
                tracing::debug!("Failed to remove {}: {e}", entry.path().display());
            }
        }
    }
}

fn refresh_access_artifacts(config: &SiteConfig, paths: &ResolvedPaths) -> Result<()> {
    if config.access.is_some() {
        access::validate_generated_file_conflicts(&paths.public_dir)?;
        images::clear_private_static_output(paths)?;
        access::write_worker(config, &paths.output)?;
    }
    Ok(())
}

fn refresh_subdomain_access_artifacts(config: &SiteConfig, paths: &ResolvedPaths) -> Result<()> {
    if config.access.is_none() {
        return Ok(());
    }

    for collection in config.subdomain_collections() {
        let mut sub_config = config.clone();
        sub_config.site.base_url = config.subdomain_base_url(collection);
        let mut sub_collection = collection.clone();
        sub_collection.url_prefix = String::new();
        sub_collection.subdomain = None;
        sub_collection.deploy_project = None;
        sub_config.collections = vec![sub_collection];

        let mut sub_paths = paths.clone();
        sub_paths.output = paths.subdomain_output(&collection.name);
        refresh_access_artifacts(&sub_config, &sub_paths)?;
    }
    Ok(())
}

/// Build each subdomain collection as an independent mini-site.
///
/// For each collection with `subdomain` set, construct a synthetic `SiteConfig`
/// with only that collection and call the build pipeline on it with a separate
/// output directory and subdomain-specific `base_url`.
fn build_subdomain_sites(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    opts: &BuildOptions,
    warnings: &mut Vec<String>,
    staged: &mut StagedOutputs,
) -> Result<Vec<SubdomainBuildInfo>> {
    let mut results = Vec::new();

    for collection in config.subdomain_collections() {
        let subdomain = collection.subdomain.as_ref().unwrap();
        let subdomain_base_url = config.subdomain_base_url(collection);
        let subdomain_output = paths.subdomain_output(&collection.name);

        // Build a synthetic config for this subdomain
        let mut sub_config = config.clone();
        sub_config.site.base_url = subdomain_base_url.clone();

        // Clear subdomain field and use root url_prefix to prevent recursion
        let mut sub_collection = collection.clone();
        sub_collection.url_prefix = String::new();
        sub_collection.subdomain = None;
        sub_collection.deploy_project = None;
        sub_config.collections = vec![sub_collection];

        // Create subdomain-specific paths
        // Subdomain outputs are always rebuilt from scratch, into staging.
        let mut sub_paths = paths.clone();
        clean_stale_build_dirs(&subdomain_output);
        sub_paths.output = staged.stage(&paths.root, &subdomain_output)?;

        // Build reverse rewrite map: links from subdomain content to other collections
        // resolve to absolute URLs on the main site (or other subdomains)
        let reverse_rewrites = config.reverse_subdomain_rewrite_map(&collection.name);

        let sub_result = build_site_inner(
            &sub_config,
            &sub_paths,
            opts,
            Some(&reverse_rewrites),
            &None,
            &subdomain_output,
        )?;
        for warning in sub_result.warnings {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }

        results.push(SubdomainBuildInfo {
            collection_name: collection.name.clone(),
            subdomain: subdomain.clone(),
            output_dir: subdomain_output,
            base_url: subdomain_base_url,
            stats: sub_result.stats,
        });
    }

    Ok(results)
}

/// Check whether the Plausible analytics config uses deprecated `extensions` without `script_url`.
fn needs_plausible_extensions_warning(analytics: Option<&AnalyticsSection>) -> bool {
    match analytics {
        Some(a) => {
            a.provider == crate::config::AnalyticsProvider::Plausible
                && !a.extensions.is_empty()
                && a.script_url.is_none()
        }
        None => false,
    }
}

/// Inner build pipeline. This is the actual 14-step build.
///
/// `subdomain_rewrites_override`: if `Some`, used instead of computing from config.
/// This allows subdomain builds to receive reverse-rewrite maps (subdomain→main site links).
///
/// `changeset`: if `Some`, used for incremental builds to skip unchanged content.
///
/// `display_output`: the final output directory. `paths.output` may be a
/// staging directory that is renamed to `display_output` after the build;
/// reported paths (e.g. broken links on generated pages) use the final one.
fn build_site_inner(
    config: &SiteConfig,
    paths: &ResolvedPaths,
    opts: &BuildOptions,
    subdomain_rewrites_override: Option<&HashMap<String, String>>,
    changeset: &Option<cache::ChangeSet>,
    display_output: &Path,
) -> Result<BuildResult> {
    let start = Instant::now();
    let mut step_timings: Vec<(String, f64)> = Vec::new();
    let mut progress = crate::progress::BuildProgress::new();

    // Determine if this is an incremental build that can skip the clean step.
    // Incremental means: changeset exists, no full rebuild needed, and output dir exists.
    // Incremental requires no deletions: we can't know the output paths for deleted items
    // without re-parsing them (frontmatter slug overrides). Fall back to full rebuild so
    // dist/ is cleaned and stale HTML files don't linger.
    let is_incremental = changeset.as_ref().is_some_and(|cs| {
        !cs.needs_full_rebuild && cs.deleted_content.is_empty() && paths.output.exists()
    });
    // Step 1: Clean output directory (skip for incremental builds)
    progress.step("Cleaning output directory");
    let step_start = Instant::now();
    if is_incremental {
        // Keep existing output — only changed items will be re-rendered
        tracing::debug!("Incremental build: keeping existing output directory");
    } else if paths.output.exists() {
        fs::remove_dir_all(&paths.output)?;
    }
    fs::create_dir_all(&paths.output)?;
    step_timings.push((
        "Clean output".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 1b: Copy public/ files to output root (before all other generation)
    progress.step("Copying public files");
    let step_start = Instant::now();
    let mut public_count: usize = 0;
    let mut public_file_paths: HashSet<String> = HashSet::new();
    if paths.public_dir.exists() {
        for entry in WalkDir::new(&paths.public_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(&paths.public_dir)
                .unwrap_or(entry.path());
            let dest = paths.output.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dest)?;
            public_file_paths.insert(rel.to_string_lossy().replace('\\', "/"));
            public_count += 1;
        }
    }
    step_timings.push((
        "Copy public files".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 2: Load templates (collection-aware)
    progress.step("Loading templates");
    let step_start = Instant::now();
    let (mut tera, mut warnings) =
        templates::load_templates_with_warnings(&paths.templates, &config.collections)?;
    for warning in &warnings {
        eprintln!("⚠ Warning: {warning}");
    }
    step_timings.push((
        "Load templates".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 2b: Load shortcode registry (built-in + user-defined)
    progress.step("Loading shortcodes");
    let step_start = Instant::now();
    let shortcodes_dir = paths.templates.join("shortcodes");
    let shortcode_registry = crate::shortcodes::ShortcodeRegistry::new(&shortcodes_dir)?;
    step_timings.push((
        "Load shortcodes".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 2.5: Load data files
    progress.step("Loading data files");
    let step_start = Instant::now();
    let data = crate::data::load_data_dir(&paths.data_dir)?;
    step_timings.push((
        "Load data files".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    let is_multilingual = config.is_multilingual();
    let default_lang = &config.site.language;

    // Register the i18n filter so templates can resolve per-language "language
    // maps" in data values (e.g. `{{ cert.description | i18n(lang=lang) }}`).
    // Then warn about partial language maps (some configured languages missing).
    let lang_codes: std::collections::HashSet<String> =
        config.all_languages().into_iter().collect();
    crate::i18n::register_filters(&mut tera, default_lang, &lang_codes);
    for warning in crate::i18n::partial_language_map_warnings(&data, &lang_codes) {
        crate::output::human::warning_stderr(&warning);
        warnings.push(warning);
    }

    // Step 3: Process each collection
    progress.step("Processing content");
    let step_start = Instant::now();
    let mut all_collections: HashMap<String, Vec<ContentItem>> = HashMap::new();

    // For incremental builds, track changed content paths to skip re-rendering unchanged items.
    // We still parse all items (needed for indexes/feeds), but skip writing unchanged pages.
    let changed_content_paths: Option<HashSet<PathBuf>> = if is_incremental {
        changeset.as_ref().map(|cs| {
            // Use absolute paths for matching against source_path
            cs.changed_content
                .iter()
                .map(|rel| paths.content.join(rel))
                .collect()
        })
    } else {
        None
    };

    // Pre-compute i18n strings per language for shortcode templates.
    // Built before the collection loop so the cache can be captured by par_iter closures.
    let sc_i18n_cache: HashMap<String, serde_json::Value> = config
        .all_languages()
        .into_iter()
        .map(|lang| {
            let t = ui_strings_for_lang(&lang, &data);
            (lang, t)
        })
        .collect();
    let sc_i18n_empty = serde_json::json!({});

    // Pre-compute shortcode site context (identical for every page)
    let sc_site = serde_json::json!({
        "title": &config.site.title,
        "base_url": &config.site.base_url,
        "language": &config.site.language,
        "contact": config.contact.as_ref().map(|c| serde_json::json!({
            "provider": serde_json::to_value(&c.provider).unwrap_or_default(),
            "endpoint": &c.endpoint,
            "region": &c.region,
            "redirect": &c.redirect,
            "subject": &c.subject,
        })),
    });

    for collection in &config.collections {
        let collection_dir = paths.content.join(&collection.directory);
        let mut items = Vec::new();

        if !collection_dir.exists() {
            tracing::warn!(
                "Content directory '{}' for collection '{}' does not exist",
                collection_dir.display(),
                collection.name
            );
        }
        if collection_dir.exists() {
            let entries: Vec<_> = WalkDir::new(&collection_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                .collect();

            let results: Vec<std::result::Result<Option<ContentItem>, PageError>> = entries
                .par_iter()
                .map(|entry| {
                    let path = entry.path();
                    let rel = path.strip_prefix(&collection_dir).unwrap_or(path);

                    let (fm, raw_body) = content::parse_content_file(path)?;

                    if fm.draft && !opts.include_drafts {
                        return Ok(None);
                    }

                    let ItemLocation { slug, url, lang } =
                        resolve_item_location(config, collection, path, rel, &fm);

                    let mut fm = fm;
                    if fm.date.is_none() && collection.has_date {
                        fm.date = parse_date_from_filename(path);
                    }

                    let sc_page = serde_json::json!({
                        "title": fm.title,
                        "slug": &slug,
                        "collection": &collection.name,
                        "tags": &fm.tags,
                    });
                    let sc_i18n = sc_i18n_cache.get(&lang).unwrap_or(&sc_i18n_empty);
                    let expanded_body =
                        shortcode_registry.expand(&raw_body, path, &sc_page, &sc_site, sc_i18n)?;
                    let excerpt = content::extract_excerpt(&expanded_body);
                    let html_input = if config.build.math {
                        math::render_math(&expanded_body)
                    } else {
                        expanded_body
                    };
                    let (html_body, toc) =
                        markdown::markdown_to_html_with(&html_input, config.build.mermaid);

                    let (excerpt_html, _) = markdown::markdown_to_html(&excerpt);
                    let word_count = raw_body.split_whitespace().count();
                    let reading_time = if word_count == 0 {
                        0
                    } else {
                        (word_count / 238).max(1)
                    };

                    Ok(Some(ContentItem {
                        frontmatter: fm,
                        raw_body,
                        html_body,
                        source_path: path.to_path_buf(),
                        slug,
                        collection: collection.name.clone(),
                        url,
                        lang,
                        excerpt,
                        toc,
                        word_count,
                        reading_time,
                        excerpt_html,
                    }))
                })
                .collect();

            for result in results {
                if let Some(item) = result? {
                    items.push(item);
                }
            }
        }

        // Sort: date-based collections by date desc, others by weight then title
        if collection.has_date {
            items.sort_by_key(|b| std::cmp::Reverse(b.frontmatter.date));
        } else {
            items.sort_by(|a, b| match (a.frontmatter.weight, b.frontmatter.weight) {
                (Some(wa), Some(wb)) => wa
                    .cmp(&wb)
                    .then_with(|| a.frontmatter.title.cmp(&b.frontmatter.title)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.frontmatter.title.cmp(&b.frontmatter.title),
            });
        }

        all_collections.insert(collection.name.clone(), items);
    }

    // Detect URL collisions: if two content items resolve to the same URL, that's an error.
    {
        let mut url_map: HashMap<&str, &std::path::Path> = HashMap::new();
        for items in all_collections.values() {
            for item in items {
                if let Some(existing) = url_map.insert(&item.url, &item.source_path) {
                    return Err(PageError::Build(format!(
                        "URL collision: '{}' is claimed by both '{}' and '{}'",
                        item.url,
                        existing.display(),
                        item.source_path.display()
                    )));
                }
            }
        }
    }

    // Rewrite links written as paths to other markdown files
    // (`[x](../docs/intro.md)`) into page URLs, now that every item's URL is
    // known. Operates on the already-rendered HTML, so content is parsed once.
    // Also records which source file each page was rendered from, so broken
    // links found in the output can be reported against the markdown source.
    let (page_sources, md_link_problems) =
        rewrite_source_links(&mut all_collections, paths, default_lang);

    step_timings.push((
        "Process collections".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Build translation map: (collection, slug) → Vec<TranslationLink>
    // Sort each translations vec by language order (default lang first, then alphabetical)
    // so the language switcher renders in a deterministic, predictable order.
    let translation_map: HashMap<(String, String), Vec<TranslationLink>> = if is_multilingual {
        let lang_order: Vec<String> = config.all_languages();
        let mut map: HashMap<(String, String), Vec<TranslationLink>> = HashMap::new();
        for (coll_name, items) in &all_collections {
            for item in items {
                let key = (coll_name.clone(), item.slug.clone());
                map.entry(key).or_default().push(TranslationLink {
                    lang: item.lang.clone(),
                    url: item.url.clone(),
                });
            }
        }
        for translations in map.values_mut() {
            translations.sort_by_key(|t| {
                lang_order
                    .iter()
                    .position(|l| *l == t.lang)
                    .unwrap_or(usize::MAX)
            });
        }
        map
    } else {
        HashMap::new()
    };

    // Render each item in each collection
    progress.step("Rendering pages");
    let step_start = Instant::now();

    // Pre-compute SiteContext per language (avoid re-creating per item)
    let mut site_ctx_cache: HashMap<String, SiteContext> = HashMap::new();
    site_ctx_cache.insert(default_lang.clone(), SiteContext::from_config(config));
    for lang in config.all_languages() {
        if lang != *default_lang {
            site_ctx_cache.insert(lang.clone(), SiteContext::for_lang(config, &lang));
        }
    }

    // Pre-compute i18n context per language (avoid re-creating 37-key JSON per page)
    let mut i18n_cache: HashMap<String, (String, String, serde_json::Value)> = HashMap::new();
    for lang in &config.all_languages() {
        let lang_prefix = lang_prefix_for(lang, default_lang);
        let t = ui_strings_for_lang(lang, &data);
        i18n_cache.insert(lang.clone(), (lang_prefix, default_lang.to_string(), t));
    }

    // Pre-serialize an empty nav for non-nested collections
    let empty_nav: Vec<NavSection> = Vec::new();
    let empty_nav_value = serde_json::to_value(&empty_nav).unwrap_or_default();

    // Collect nav data per collection so it can be passed to collection index templates later.
    // Key: collection name → (lang → serialized nav). Only populated for nested collections.
    let mut collection_nav_cache: HashMap<String, HashMap<String, serde_json::Value>> =
        HashMap::new();

    for collection in &config.collections {
        if let Some(items) = all_collections.get(&collection.name) {
            // Only build nav for nested collections (e.g., docs with sidebar).
            // Non-nested collections (posts, pages) get an empty nav — their templates
            // don't use it, and building/cloning a 10k-item nav per page is O(n²).
            let nav_by_lang: HashMap<&str, serde_json::Value>;
            let nav_slug_index: HashMap<&str, HashMap<&str, (usize, usize)>>;

            if collection.nested {
                // Group items by language
                let mut items_by_lang: HashMap<&str, Vec<&ContentItem>> = HashMap::new();
                for item in items {
                    items_by_lang
                        .entry(item.lang.as_str())
                        .or_default()
                        .push(item);
                }

                let mut by_lang = HashMap::new();
                let mut slug_idx = HashMap::new();

                for (&lang, lang_items) in &items_by_lang {
                    let mut sections: Vec<NavSection> = Vec::new();
                    let mut section_map: HashMap<String, usize> = HashMap::new();
                    let mut si: HashMap<&str, (usize, usize)> = HashMap::new();

                    for item in lang_items {
                        let (section_name, section_label) = if let Some(pos) = item.slug.find('/') {
                            let name = &item.slug[..pos];
                            let label = title_case(name);
                            (name.to_string(), label)
                        } else {
                            (String::new(), String::new())
                        };

                        let nav_item = NavItem {
                            title: item.frontmatter.title.clone(),
                            url: item.url.clone(),
                            active: false,
                        };

                        let section_idx = if let Some(&idx) = section_map.get(&section_name) {
                            sections[idx].items.push(nav_item);
                            idx
                        } else {
                            let idx = sections.len();
                            section_map.insert(section_name.clone(), idx);
                            sections.push(NavSection {
                                name: section_name,
                                label: section_label,
                                items: vec![nav_item],
                            });
                            idx
                        };
                        let item_idx = sections[section_idx].items.len() - 1;
                        si.insert(&item.slug, (section_idx, item_idx));
                    }

                    // Pre-serialize to serde_json::Value so per-item clone is cheap
                    by_lang.insert(lang, serde_json::to_value(&sections).unwrap_or_default());
                    slug_idx.insert(lang, si);
                }

                nav_by_lang = by_lang;
                nav_slug_index = slug_idx;
            } else {
                nav_by_lang = HashMap::new();
                nav_slug_index = HashMap::new();
            }

            // Cache nav for collection index rendering later (Steps 4b/4b-extra)
            if collection.nested && !nav_by_lang.is_empty() {
                let owned: HashMap<String, serde_json::Value> = nav_by_lang
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect();
                collection_nav_cache.insert(collection.name.clone(), owned);
            }

            // Pre-compute adjacent (prev/next) posts per language for this collection.
            // Items are already sorted (date desc for dated, weight/title for others).
            // "Next" = newer (toward index 0), "Previous" = older (toward end).
            let adjacent_posts: HashMap<
                (String, String),
                (Option<AdjacentPost>, Option<AdjacentPost>),
            > = {
                let mut map = HashMap::new();
                // Group items by language to compute adjacency within same language
                let mut by_lang: HashMap<&str, Vec<usize>> = HashMap::new();
                for (i, item) in items.iter().enumerate() {
                    by_lang.entry(item.lang.as_str()).or_default().push(i);
                }
                for indices in by_lang.values() {
                    for (pos, &idx) in indices.iter().enumerate() {
                        let item = &items[idx];
                        // Next = newer post (previous index in sorted-desc list)
                        let next = if pos > 0 {
                            let ni = indices[pos - 1];
                            let n = &items[ni];
                            Some(AdjacentPost {
                                title: n.frontmatter.title.clone(),
                                url: n.url.clone(),
                                date: n.frontmatter.date.map(|d| d.to_string()),
                                description: n.frontmatter.description.clone(),
                            })
                        } else {
                            None
                        };
                        // Prev = older post (next index in sorted-desc list)
                        let prev = if pos + 1 < indices.len() {
                            let pi = indices[pos + 1];
                            let p = &items[pi];
                            Some(AdjacentPost {
                                title: p.frontmatter.title.clone(),
                                url: p.url.clone(),
                                date: p.frontmatter.date.map(|d| d.to_string()),
                                description: p.frontmatter.description.clone(),
                            })
                        } else {
                            None
                        };
                        map.insert((item.lang.clone(), item.slug.clone()), (prev, next));
                    }
                }
                map
            };

            // If any item in this collection changed, re-render the whole collection.
            // This keeps prev/next links and translation links correct: those values
            // depend on sibling items, so a change to one post can affect its neighbors.
            let collection_has_changes = changed_content_paths.as_ref().is_some_and(|changed| {
                items.iter().any(|item| changed.contains(&item.source_path))
            });

            let render_results: Vec<std::result::Result<Option<(PathBuf, String)>, PageError>> =
                items
                    .par_iter()
                    .map(|item| {
                        // In incremental mode, skip rendering items whose source hasn't changed,
                        // UNLESS any sibling in this collection changed (which can affect
                        // prev/next links and translation links for all items in the collection).
                        if !collection_has_changes {
                            if let Some(ref changed) = changed_content_paths {
                                if !changed.contains(&item.source_path) {
                                    return Ok(None);
                                }
                            }
                        }

                        let site_ctx_for_item =
                            site_ctx_cache.get(item.lang.as_str()).unwrap_or_else(|| {
                                site_ctx_cache
                                    .get(default_lang.as_str())
                                    .expect("default language missing from site context cache")
                            });

                        let mut ctx =
                            build_page_context(site_ctx_for_item, item, &data, collection.private);

                        // Inject adjacent post links (prev_post / next_post).
                        // Always insert both keys so Tera templates can check them without errors.
                        let empty: (Option<AdjacentPost>, Option<AdjacentPost>) = (None, None);
                        let (prev, next) = adjacent_posts
                            .get(&(item.lang.clone(), item.slug.clone()))
                            .unwrap_or(&empty);
                        ctx.insert("prev_post", prev);
                        ctx.insert("next_post", next);

                        if collection.nested {
                            if let Some(base_nav) = nav_by_lang.get(item.lang.as_str()) {
                                let mut nav = base_nav.clone();
                                if let Some(si) = nav_slug_index.get(item.lang.as_str()) {
                                    if let Some(&(sec_idx, item_idx)) = si.get(item.slug.as_str()) {
                                        if let Some(sections) = nav.as_array_mut() {
                                            if let Some(section) = sections.get_mut(sec_idx) {
                                                if let Some(items_arr) = section
                                                    .get_mut("items")
                                                    .and_then(|i| i.as_array_mut())
                                                {
                                                    if let Some(nav_item) =
                                                        items_arr.get_mut(item_idx)
                                                    {
                                                        nav_item["active"] =
                                                            serde_json::Value::Bool(true);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                ctx.insert("nav", &nav);
                            }
                        } else {
                            ctx.insert("nav", &empty_nav_value);
                        }
                        ctx.insert("lang", &item.lang);
                        if let Some(cached_i18n) = i18n_cache.get(item.lang.as_str()) {
                            insert_i18n_context_cached(&mut ctx, cached_i18n);
                        } else {
                            insert_i18n_context(&mut ctx, &item.lang, default_lang, &data);
                        }
                        insert_build_flags(&mut ctx, config);

                        let empty_translations: Vec<TranslationLink> = Vec::new();
                        let translations = translation_map
                            .get(&(collection.name.clone(), item.slug.clone()))
                            .filter(|t| t.len() > 1)
                            .map(|t| t.as_slice())
                            .unwrap_or(&empty_translations);
                        ctx.insert("translations", &translations);

                        let template_name = item
                            .frontmatter
                            .template
                            .as_deref()
                            .unwrap_or(&collection.default_template);
                        let html = tera.render(template_name, &ctx).map_err(|e| {
                            use std::error::Error as _;
                            let mut source_chain = String::new();
                            let mut source: Option<&dyn std::error::Error> = e.source();
                            while let Some(s) = source {
                                source_chain.push_str(&format!("\n  Caused by: {s}"));
                                source = s.source();
                            }
                            PageError::Build(format!(
                                "rendering '{}': {e}{source_chain}",
                                item.slug
                            ))
                        })?;

                        let output_path = url_to_output_path(&paths.output, &item.url);
                        Ok(Some((output_path, html)))
                    })
                    .collect();

            for result in render_results {
                if let Some((output_path, html)) = result? {
                    if let Some(parent) = output_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&output_path, html)?;
                }
            }
        }
    }

    step_timings.push((
        "Render pages".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Extract homepage pages (content/pages/index.md and translations) so they
    // don't also render as standalone pages at /index (which would collide).
    let homepage_pages: Vec<ContentItem> = all_collections
        .get_mut("pages")
        .map(|pages| {
            let mut homepages = Vec::new();
            let mut i = 0;
            while i < pages.len() {
                if pages[i].slug == "index" {
                    homepages.push(pages.remove(i));
                } else {
                    i += 1;
                }
            }
            homepages
        })
        .unwrap_or_default();

    // Extract collection index pages (content/{collection}/index.md and translations)
    // so they don't render as standalone items but instead inject content into the
    // collection's index page — the same pattern as content/pages/index.md for the
    // site homepage. This also powers subdomain root pages: when a collection is
    // deployed to its own subdomain, its index.md becomes the subdomain root content.
    let mut collection_index_pages: HashMap<String, Vec<ContentItem>> = HashMap::new();
    for (name, items) in all_collections.iter_mut() {
        if name == "pages" {
            continue; // pages/index.md is already handled above as the site homepage
        }
        let mut index_items = Vec::new();
        let mut i = 0;
        while i < items.len() {
            if items[i].slug == "index" {
                index_items.push(items.remove(i));
            } else {
                i += 1;
            }
        }
        if !index_items.is_empty() {
            collection_index_pages.insert(name.clone(), index_items);
        }
    }

    // Step 4: Render index page(s)
    progress.step("Rendering indexes");
    let step_start = Instant::now();
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        let mut index_ctx = tera::Context::new();
        index_ctx.insert("site", &lang_site_ctx);
        index_ctx.insert("data", &data);
        index_ctx.insert("lang", lang);
        insert_i18n_context(&mut index_ctx, lang, default_lang, &data);
        insert_build_flags(&mut index_ctx, config);

        // A collection that serves at the site root (empty url_prefix) and has its
        // own index template is a subdomain hub. Render it exactly like the
        // main-domain collection index — the collection's own template, with its
        // own collection in `collections` (unfiltered by listed/private) — instead
        // of the generic site index template.
        let subdomain_hub = config.collections.iter().find(|c| {
            c.url_prefix.is_empty() && tera.get_template(&format!("{}-index.html", c.name)).is_ok()
        });
        let index_template = match subdomain_hub {
            Some(c) => format!("{}-index.html", c.name),
            None => "index.html".to_string(),
        };

        // Collections for this language: the hub's own collection for a subdomain
        // root, otherwise the listed, non-private collections.
        let index_collections: Vec<&CollectionConfig> = match subdomain_hub {
            Some(hub) => vec![hub],
            None => config
                .collections
                .iter()
                .filter(|c| c.listed && !c.private)
                .collect(),
        };
        let collections_ctx: Vec<CollectionContext> = index_collections
            .iter()
            .map(|&c| {
                let items = all_collections
                    .get(&c.name)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|item| item.lang == *lang)
                    .collect::<Vec<_>>();
                CollectionContext {
                    name: c.name.clone(),
                    label: c.label.clone(),
                    items: items
                        .iter()
                        .map(|item| ItemSummary {
                            title: item.frontmatter.title.clone(),
                            date: item.frontmatter.date.map(|d| d.to_string()),
                            description: item.frontmatter.description.clone(),
                            slug: item.slug.clone(),
                            tags: item.frontmatter.tags.clone(),
                            url: item.url.clone(),
                            word_count: item.word_count,
                            reading_time: item.reading_time,
                            excerpt: item.excerpt_html.clone(),
                        })
                        .collect(),
                }
            })
            .collect();
        // The main-domain collection index also exposes `items`; mirror it so
        // listing-style hubs (e.g. docs) render identically on a subdomain.
        if subdomain_hub.is_some() {
            let hub_items: Vec<ItemSummary> = collections_ctx
                .first()
                .map(|c| c.items.clone())
                .unwrap_or_default();
            index_ctx.insert("items", &hub_items);
        }
        index_ctx.insert("collections", &collections_ctx);

        // For subdomain builds (single collection with empty url_prefix), pass the nav
        // so the docs theme sidebar works on the root index page.
        if let Some(col) = config.collections.first() {
            if col.url_prefix.is_empty() {
                if let Some(nav_langs) = collection_nav_cache.get(&col.name) {
                    if let Some(nav_val) = nav_langs.get(lang.as_str()) {
                        index_ctx.insert("nav", nav_val);
                    }
                }
            }
        }

        // Always insert a `page` context so templates can unconditionally access
        // page.url, page.collection, etc. for SEO/GEO meta tags.
        // When no homepage page exists, `page.content` is empty so
        // `{% if page.content %}` gates the homepage content block correctly.
        let index_page_url = if *lang == *default_lang {
            "/".to_string()
        } else {
            format!("/{lang}/")
        };
        let index_page_ctx = if let Some(homepage) = homepage_pages.iter().find(|p| p.lang == *lang)
        {
            PageContext {
                title: homepage.frontmatter.title.clone(),
                content: homepage.html_body.clone(),
                date: homepage.frontmatter.date.map(|d| d.to_string()),
                updated: homepage.frontmatter.updated.map(|d| d.to_string()),
                description: homepage.frontmatter.description.clone(),
                image: absolutize_image(
                    homepage.frontmatter.image.as_deref(),
                    &config.site.base_url,
                ),
                slug: homepage.slug.clone(),
                tags: homepage.frontmatter.tags.clone(),
                url: index_page_url,
                collection: homepage.collection.clone(),
                robots: homepage.frontmatter.robots.clone(),
                word_count: homepage.word_count,
                reading_time: homepage.reading_time,
                excerpt: homepage.excerpt_html.clone(),
                toc: homepage.toc.clone(),
                extra: homepage.frontmatter.extra.clone(),
            }
        } else if let Some(col_index) = config
            .collections
            .iter()
            .find(|c| c.url_prefix.is_empty())
            .and_then(|c| {
                collection_index_pages
                    .get(&c.name)
                    .and_then(|pages| pages.iter().find(|p| p.lang == *lang))
            })
        {
            // Subdomain root: use the collection's index.md as homepage content
            PageContext {
                title: col_index.frontmatter.title.clone(),
                content: col_index.html_body.clone(),
                date: col_index.frontmatter.date.map(|d| d.to_string()),
                updated: col_index.frontmatter.updated.map(|d| d.to_string()),
                description: col_index.frontmatter.description.clone(),
                image: absolutize_image(
                    col_index.frontmatter.image.as_deref(),
                    &config.site.base_url,
                ),
                slug: col_index.slug.clone(),
                tags: col_index.frontmatter.tags.clone(),
                url: index_page_url,
                collection: col_index.collection.clone(),
                robots: col_index.frontmatter.robots.clone(),
                word_count: col_index.word_count,
                reading_time: col_index.reading_time,
                excerpt: col_index.excerpt_html.clone(),
                toc: col_index.toc.clone(),
                extra: col_index.frontmatter.extra.clone(),
            }
        } else {
            PageContext {
                title: String::new(),
                content: String::new(),
                date: None,
                updated: None,
                description: None,
                image: None,
                slug: "index".to_string(),
                tags: Vec::new(),
                url: index_page_url,
                collection: String::new(),
                robots: None,
                word_count: 0,
                reading_time: 0,
                excerpt: String::new(),
                toc: Vec::new(),
                extra: std::collections::HashMap::new(),
            }
        };
        index_ctx.insert("page", &index_page_ctx);

        // Translation links for the index page (always provide, even if empty)
        let index_translations: Vec<TranslationLink> = if is_multilingual {
            let translations: Vec<TranslationLink> = config
                .all_languages()
                .iter()
                .filter(|l| homepage_pages.iter().any(|p| p.lang == **l) || **l == *default_lang)
                .map(|l| TranslationLink {
                    lang: l.clone(),
                    url: if *l == *default_lang {
                        "/".to_string()
                    } else {
                        format!("/{l}/")
                    },
                })
                .collect();
            if translations.len() > 1 {
                translations
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        index_ctx.insert("translations", &index_translations);

        // Handle redirect_to on collection index.md used as subdomain root
        let redirect_target = config
            .collections
            .iter()
            .find(|c| c.url_prefix.is_empty())
            .and_then(|c| {
                collection_index_pages
                    .get(&c.name)
                    .and_then(|pages| pages.iter().find(|p| p.lang == *lang))
            })
            .and_then(|ci| ci.frontmatter.extra.get("redirect_to"))
            .and_then(|v| v.as_str().map(String::from));

        let index_html = if let Some(target) = redirect_target {
            let target_url = if target.starts_with('/') {
                format!("{}{target}", lang_prefix_for(lang, default_lang))
            } else {
                target
            };
            generate_redirect_html(&target_url)
        } else {
            tera.render(&index_template, &index_ctx)
                .map_err(|e| PageError::Build(format!("rendering index ({lang}): {e}")))?
        };

        if *lang == *default_lang {
            fs::write(paths.output.join("index.html"), index_html)?;
        } else {
            let lang_dir = paths.output.join(lang);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("index.html"), index_html)?;
        }
    }

    // Step 4b: Paginated collection index pages
    // For each listed collection with `paginate` set, render per-language paginated indexes.
    // Page 1 → dist/{url_prefix}/index.html  (URL: /{url_prefix}/)
    // Page N → dist/{url_prefix}/page/N/index.html  (URL: /{url_prefix}/page/N/)
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        for c in config.collections.iter().filter(|c| c.listed || c.private) {
            let page_size = match c.paginate {
                Some(n) if n > 0 => n,
                _ => continue,
            };
            let url_prefix_trimmed = c.url_prefix.trim_start_matches('/');
            if url_prefix_trimmed.is_empty() {
                continue;
            }

            let all_items: Vec<&ContentItem> = all_collections
                .get(&c.name)
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let lang_items: Vec<ItemSummary> = all_items
                .iter()
                .filter(|item| item.lang == *lang)
                .map(|item| ItemSummary {
                    title: item.frontmatter.title.clone(),
                    date: item.frontmatter.date.map(|d| d.to_string()),
                    description: item.frontmatter.description.clone(),
                    slug: item.slug.clone(),
                    tags: item.frontmatter.tags.clone(),
                    url: item.url.clone(),
                    word_count: item.word_count,
                    reading_time: item.reading_time,
                    excerpt: item.excerpt_html.clone(),
                })
                .collect();

            let total = lang_items.len();
            if total == 0 {
                continue;
            }
            let total_pages = total.div_ceil(page_size);

            // Base collection URL (language-prefixed for non-default languages)
            let collection_base = if *lang == *default_lang {
                format!("/{url_prefix_trimmed}")
            } else {
                format!("/{lang}/{url_prefix_trimmed}")
            };

            let page_url = |n: usize| -> String {
                if n == 1 {
                    format!("{collection_base}/")
                } else {
                    format!("{collection_base}/page/{n}/")
                }
            };

            for (page_idx, chunk) in lang_items.chunks(page_size).enumerate() {
                let page_num = page_idx + 1;
                let collection_ctx = CollectionContext {
                    name: c.name.clone(),
                    label: c.label.clone(),
                    items: chunk.to_vec(),
                };
                let pagination = PaginationContext {
                    current_page: page_num,
                    total_pages,
                    prev_url: if page_num > 1 {
                        Some(page_url(page_num - 1))
                    } else {
                        None
                    },
                    next_url: if page_num < total_pages {
                        Some(page_url(page_num + 1))
                    } else {
                        None
                    },
                    base_url: collection_base.clone(),
                };
                let mut ctx = tera::Context::new();
                ctx.insert("site", &lang_site_ctx);
                ctx.insert("data", &data);
                ctx.insert("lang", lang);
                insert_i18n_context(&mut ctx, lang, default_lang, &data);
                insert_build_flags(&mut ctx, config);
                ctx.insert("collections", &[collection_ctx]);
                ctx.insert("pagination", &pagination);
                ctx.insert("translations", &Vec::<TranslationLink>::new());

                // Pass sidebar nav for nested collections.
                // If this language has items (guaranteed — we skip empty above), its
                // nav was built from those same items, so the lookup always succeeds
                // when the collection is in the cache.
                let nav_val = collection_nav_cache
                    .get(&c.name)
                    .and_then(|langs| langs.get(lang.as_str()))
                    .unwrap_or(&empty_nav_value);
                ctx.insert("nav", nav_val);

                // Always provide a page context so SEO meta tags can access page.url etc.
                // Insert items directly so collection-specific index templates can use {% for item in items %}
                ctx.insert("items", &chunk.to_vec());

                // On page 1, inject collection's index.md content if available
                let col_index_page = if page_num == 1 {
                    collection_index_pages
                        .get(&c.name)
                        .and_then(|pages| pages.iter().find(|p| p.lang == *lang))
                } else {
                    None
                };
                ctx.insert(
                    "page",
                    &if let Some(ci) = col_index_page {
                        PageContext {
                            title: ci.frontmatter.title.clone(),
                            content: ci.html_body.clone(),
                            date: ci.frontmatter.date.map(|d| d.to_string()),
                            updated: ci.frontmatter.updated.map(|d| d.to_string()),
                            description: ci.frontmatter.description.clone(),
                            image: absolutize_image(
                                ci.frontmatter.image.as_deref(),
                                &config.site.base_url,
                            ),
                            slug: url_prefix_trimmed.to_string(),
                            tags: ci.frontmatter.tags.clone(),
                            url: page_url(page_num),
                            collection: c.name.clone(),
                            robots: effective_robots(ci.frontmatter.robots.clone(), c.private),
                            word_count: ci.word_count,
                            reading_time: ci.reading_time,
                            excerpt: ci.excerpt_html.clone(),
                            toc: ci.toc.clone(),
                            extra: ci.frontmatter.extra.clone(),
                        }
                    } else {
                        PageContext {
                            title: String::new(),
                            content: String::new(),
                            date: None,
                            updated: None,
                            description: None,
                            image: None,
                            slug: String::new(),
                            tags: Vec::new(),
                            url: page_url(page_num),
                            collection: String::new(),
                            robots: effective_robots(None, c.private),
                            word_count: 0,
                            reading_time: 0,
                            excerpt: String::new(),
                            toc: Vec::new(),
                            extra: std::collections::HashMap::new(),
                        }
                    },
                );
                // Use collection-specific index template if available (e.g., changelog-index.html)
                let collection_index_template = format!("{}-index.html", c.name);
                let template_name = if tera.get_template(&collection_index_template).is_ok() {
                    &collection_index_template
                } else {
                    "index.html"
                };
                let html = tera.render(template_name, &ctx).map_err(|e| {
                    PageError::Build(format!("rendering {collection_base} page {page_num}: {e}"))
                })?;

                let out_dir = if *lang == *default_lang {
                    if page_num == 1 {
                        paths.output.join(url_prefix_trimmed)
                    } else {
                        paths
                            .output
                            .join(url_prefix_trimmed)
                            .join("page")
                            .join(page_num.to_string())
                    }
                } else if page_num == 1 {
                    paths.output.join(lang).join(url_prefix_trimmed)
                } else {
                    paths
                        .output
                        .join(lang)
                        .join(url_prefix_trimmed)
                        .join("page")
                        .join(page_num.to_string())
                };
                fs::create_dir_all(&out_dir)?;
                fs::write(out_dir.join("index.html"), html)?;
            }

            // Write markdown index (all items, not just one page) alongside page 1
            let md_content = generate_collection_index_md(&c.label, &lang_items);
            let md_dir = if *lang == *default_lang {
                paths.output.join(url_prefix_trimmed)
            } else {
                paths.output.join(lang).join(url_prefix_trimmed)
            };
            fs::write(md_dir.join("index.md"), md_content)?;
        }
    }

    // Step 4b-extra: Non-paginated collection index pages
    // For listed collections with a url_prefix but no `paginate`, generate a single index page
    // at /{url_prefix}/ using the collection-specific index template (or index.html fallback).
    for lang in &config.all_languages() {
        let lang_site_ctx = SiteContext::for_lang(config, lang);
        for c in config
            .collections
            .iter()
            .filter(|c| (c.listed || c.private) && c.paginate.is_none() && !c.url_prefix.is_empty())
        {
            let url_prefix_trimmed = c.url_prefix.trim_start_matches('/');
            let collection_url = if *lang == *default_lang {
                format!("/{url_prefix_trimmed}/")
            } else {
                format!("/{lang}/{url_prefix_trimmed}/")
            };

            let lang_items: Vec<ItemSummary> = all_collections
                .get(&c.name)
                .map(|v| {
                    v.iter()
                        .filter(|item| item.lang == *lang)
                        .map(|item| ItemSummary {
                            title: item.frontmatter.title.clone(),
                            date: item.frontmatter.date.map(|d| d.to_string()),
                            description: item.frontmatter.description.clone(),
                            slug: item.slug.clone(),
                            tags: item.frontmatter.tags.clone(),
                            url: item.url.clone(),
                            word_count: item.word_count,
                            reading_time: item.reading_time,
                            excerpt: item.excerpt_html.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let collection_ctx = CollectionContext {
                name: c.name.clone(),
                label: c.label.clone(),
                items: lang_items.clone(),
            };

            let mut ctx = tera::Context::new();
            ctx.insert("site", &lang_site_ctx);
            ctx.insert("data", &data);
            ctx.insert("lang", lang);
            insert_i18n_context(&mut ctx, lang, default_lang, &data);
            insert_build_flags(&mut ctx, config);
            ctx.insert("collections", &[collection_ctx]);
            ctx.insert("items", &lang_items);
            ctx.insert("translations", &Vec::<TranslationLink>::new());

            // Pass sidebar nav for nested collections (e.g., docs) so the
            // collection index template can render the same sidebar as individual pages.
            let nav_val = collection_nav_cache
                .get(&c.name)
                .and_then(|langs| langs.get(lang.as_str()))
                .unwrap_or(&empty_nav_value);
            ctx.insert("nav", nav_val);

            // Use collection's index.md content if available (content/{collection}/index.md)
            let col_index_page = collection_index_pages
                .get(&c.name)
                .and_then(|pages| pages.iter().find(|p| p.lang == *lang));

            // Support redirect_to in collection index.md: generate an HTML redirect
            // instead of the normal index template.
            if let Some(ci) = col_index_page {
                if let Some(redirect_url) = ci.frontmatter.extra.get("redirect_to") {
                    if let Some(target) = redirect_url.as_str() {
                        let target_url = if target.starts_with('/') {
                            format!("{}{target}", lang_prefix_for(lang, default_lang))
                        } else {
                            target.to_string()
                        };
                        let redirect_html = generate_redirect_html(&target_url);
                        let out_dir = if *lang == *default_lang {
                            paths.output.join(url_prefix_trimmed)
                        } else {
                            paths.output.join(lang).join(url_prefix_trimmed)
                        };
                        fs::create_dir_all(&out_dir)?;
                        fs::write(out_dir.join("index.html"), redirect_html)?;
                        continue;
                    }
                }
            }

            ctx.insert(
                "page",
                &if let Some(ci) = col_index_page {
                    PageContext {
                        title: ci.frontmatter.title.clone(),
                        content: ci.html_body.clone(),
                        date: ci.frontmatter.date.map(|d| d.to_string()),
                        updated: ci.frontmatter.updated.map(|d| d.to_string()),
                        description: ci.frontmatter.description.clone(),
                        image: absolutize_image(
                            ci.frontmatter.image.as_deref(),
                            &config.site.base_url,
                        ),
                        slug: url_prefix_trimmed.to_string(),
                        tags: ci.frontmatter.tags.clone(),
                        url: collection_url.clone(),
                        collection: c.name.clone(),
                        robots: effective_robots(ci.frontmatter.robots.clone(), c.private),
                        word_count: ci.word_count,
                        reading_time: ci.reading_time,
                        excerpt: ci.excerpt_html.clone(),
                        toc: ci.toc.clone(),
                        extra: ci.frontmatter.extra.clone(),
                    }
                } else {
                    PageContext {
                        title: c.label.clone(),
                        content: String::new(),
                        date: None,
                        updated: None,
                        description: None,
                        image: None,
                        slug: url_prefix_trimmed.to_string(),
                        tags: Vec::new(),
                        url: collection_url.clone(),
                        collection: c.name.clone(),
                        robots: effective_robots(None, c.private),
                        word_count: 0,
                        reading_time: 0,
                        excerpt: String::new(),
                        toc: Vec::new(),
                        extra: std::collections::HashMap::new(),
                    }
                },
            );

            let collection_index_template = format!("{}-index.html", c.name);
            let template_name = if tera.get_template(&collection_index_template).is_ok() {
                &collection_index_template
            } else {
                "index.html"
            };
            let html = tera
                .render(template_name, &ctx)
                .map_err(|e| PageError::Build(format!("rendering {collection_url}: {e}")))?;

            let out_dir = if *lang == *default_lang {
                paths.output.join(url_prefix_trimmed)
            } else {
                paths.output.join(lang).join(url_prefix_trimmed)
            };
            fs::create_dir_all(&out_dir)?;
            fs::write(out_dir.join("index.html"), html)?;

            // Write markdown index alongside HTML
            let md_content = generate_collection_index_md(&c.label, &lang_items);
            fs::write(out_dir.join("index.md"), md_content)?;
        }
    }

    // Write homepage markdown alongside HTML
    for homepage in &homepage_pages {
        let md_content = format!(
            "{}\n\n{}",
            content::generate_frontmatter(&homepage.frontmatter),
            homepage.raw_body
        );
        if homepage.lang == *default_lang {
            fs::write(paths.output.join("index.md"), md_content)?;
        } else {
            let lang_dir = paths.output.join(&homepage.lang);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("index.md"), md_content)?;
        }
    }

    // Step 4b: Generate 404 page (per-language for multilingual sites)
    if tera.get_template("404.html").is_ok() {
        for lang in &config.all_languages() {
            let lang_prefix = lang_prefix_for(lang, default_lang);
            let t = ui_strings_for_lang(lang, &data);
            let not_found_title = t
                .get("not_found_title")
                .and_then(|v| v.as_str())
                .unwrap_or("Page Not Found")
                .to_string();

            let mut ctx_404 = tera::Context::new();
            ctx_404.insert("site", &SiteContext::for_lang(config, lang));
            ctx_404.insert("data", &data);
            ctx_404.insert("lang", lang);
            insert_i18n_context(&mut ctx_404, lang, default_lang, &data);
            insert_build_flags(&mut ctx_404, config);
            ctx_404.insert("translations", &Vec::<TranslationLink>::new());
            ctx_404.insert("collections", &Vec::<CollectionContext>::new());
            ctx_404.insert(
                "page",
                &PageContext {
                    title: not_found_title,
                    content: String::new(),
                    date: None,
                    updated: None,
                    description: None,
                    image: None,
                    slug: "404".to_string(),
                    tags: Vec::new(),
                    url: format!("{lang_prefix}/404"),
                    collection: String::new(),
                    robots: Some("noindex".to_string()),
                    word_count: 0,
                    reading_time: 0,
                    excerpt: String::new(),
                    toc: Vec::new(),
                    extra: std::collections::HashMap::new(),
                },
            );
            let html_404 = tera
                .render("404.html", &ctx_404)
                .map_err(|e| PageError::Build(format!("rendering 404 page ({lang}): {e}")))?;

            if *lang == *default_lang {
                fs::write(paths.output.join("404.html"), html_404)?;
            } else {
                let lang_dir = paths.output.join(lang);
                fs::create_dir_all(&lang_dir)?;
                fs::write(lang_dir.join("404.html"), html_404)?;
            }
        }
    }

    // Step 4c: Generate tag pages
    // Collect all tags per language from all collections
    let mut tag_page_urls: Vec<String> = Vec::new();
    if tera.get_template("tags.html").is_ok() && tera.get_template("tag.html").is_ok() {
        for lang in &config.all_languages() {
            let lang_prefix = if *lang == *default_lang {
                String::new()
            } else {
                format!("/{lang}")
            };
            let tags_base_url = format!("{lang_prefix}/tags");

            // Gather all tags and their items for this language
            let mut tag_map: HashMap<String, Vec<ItemSummary>> = HashMap::new();
            for c in &config.collections {
                // Private collections never contribute to public tag pages.
                if c.private {
                    continue;
                }
                if let Some(items) = all_collections.get(&c.name) {
                    for item in items.iter().filter(|i| i.lang == *lang) {
                        let summary = ItemSummary {
                            title: item.frontmatter.title.clone(),
                            date: item.frontmatter.date.map(|d| d.to_string()),
                            description: item.frontmatter.description.clone(),
                            slug: item.slug.clone(),
                            tags: item.frontmatter.tags.clone(),
                            url: item.url.clone(),
                            word_count: item.word_count,
                            reading_time: item.reading_time,
                            excerpt: item.excerpt_html.clone(),
                        };
                        for tag in &item.frontmatter.tags {
                            let normalized = tag.to_lowercase();
                            tag_map.entry(normalized).or_default().push(summary.clone());
                        }
                    }
                }
            }

            if tag_map.is_empty() {
                continue;
            }

            // Sort tags alphabetically
            let mut sorted_tags: Vec<(String, Vec<ItemSummary>)> = tag_map.into_iter().collect();
            sorted_tags.sort_by(|a, b| a.0.cmp(&b.0));

            let lang_site_ctx = SiteContext::for_lang(config, lang);

            // Generate tags index page
            #[derive(Serialize)]
            struct TagIndexEntry {
                name: String,
                url: String,
                count: usize,
            }

            let tag_entries: Vec<TagIndexEntry> = sorted_tags
                .iter()
                .map(|(tag, items)| {
                    let tag_slug = slug::slugify(tag);
                    TagIndexEntry {
                        name: tag.clone(),
                        url: format!("{tags_base_url}/{tag_slug}/"),
                        count: items.len(),
                    }
                })
                .collect();

            let mut tags_ctx = tera::Context::new();
            tags_ctx.insert("site", &lang_site_ctx);
            tags_ctx.insert("data", &data);
            tags_ctx.insert("lang", lang);
            insert_i18n_context(&mut tags_ctx, lang, default_lang, &data);
            insert_build_flags(&mut tags_ctx, config);
            tags_ctx.insert("tags", &tag_entries);
            tags_ctx.insert("translations", &Vec::<TranslationLink>::new());
            tags_ctx.insert(
                "page",
                &PageContext {
                    title: "Tags".to_string(),
                    content: String::new(),
                    date: None,
                    updated: None,
                    description: None,
                    image: None,
                    slug: "tags".to_string(),
                    tags: Vec::new(),
                    url: format!("{tags_base_url}/"),
                    collection: String::new(),
                    robots: None,
                    word_count: 0,
                    reading_time: 0,
                    excerpt: String::new(),
                    toc: Vec::new(),
                    extra: std::collections::HashMap::new(),
                },
            );
            let tags_html = tera
                .render("tags.html", &tags_ctx)
                .map_err(|e| PageError::Build(format!("rendering tags index: {e}")))?;
            let tags_dir = paths.output.join(tags_base_url.trim_start_matches('/'));
            fs::create_dir_all(&tags_dir)?;
            fs::write(tags_dir.join("index.html"), tags_html)?;
            tag_page_urls.push(format!("{tags_base_url}/"));

            // Generate individual tag pages
            for (tag, items) in &sorted_tags {
                let tag_slug = slug::slugify(tag);
                let tag_url = format!("{tags_base_url}/{tag_slug}/");
                let mut tag_ctx = tera::Context::new();
                tag_ctx.insert("site", &lang_site_ctx);
                tag_ctx.insert("data", &data);
                tag_ctx.insert("lang", lang);
                insert_i18n_context(&mut tag_ctx, lang, default_lang, &data);
                insert_build_flags(&mut tag_ctx, config);
                tag_ctx.insert("tag_name", tag);
                tag_ctx.insert("items", items);
                tag_ctx.insert("tags_url", &format!("{tags_base_url}/"));
                tag_ctx.insert("translations", &Vec::<TranslationLink>::new());
                tag_ctx.insert(
                    "page",
                    &PageContext {
                        title: format!("Tag: {tag}"),
                        content: String::new(),
                        date: None,
                        updated: None,
                        description: None,
                        image: None,
                        slug: format!("tags/{tag_slug}"),
                        tags: Vec::new(),
                        url: tag_url.clone(),
                        collection: String::new(),
                        robots: None,
                        word_count: 0,
                        reading_time: 0,
                        excerpt: String::new(),
                        toc: Vec::new(),
                        extra: std::collections::HashMap::new(),
                    },
                );
                let tag_html = tera
                    .render("tag.html", &tag_ctx)
                    .map_err(|e| PageError::Build(format!("rendering tag '{tag}': {e}")))?;
                let tag_dir = paths.output.join(format!(
                    "{}/{tag_slug}",
                    tags_base_url.trim_start_matches('/')
                ));
                fs::create_dir_all(&tag_dir)?;
                fs::write(tag_dir.join("index.html"), tag_html)?;
                tag_page_urls.push(tag_url);
            }
        }
    }

    step_timings.push((
        "Render indexes".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 5: Generate RSS and Atom feeds
    progress.step("Generating feeds");
    let step_start = Instant::now();
    let base = config.site.base_url.trim_end_matches('/');

    // Default language feeds at /feed.xml and /atom.xml
    let default_feed_items: Vec<&ContentItem> = config
        .collections
        .iter()
        .filter(|c| c.has_rss && !c.private)
        .flat_map(|c| all_collections.get(&c.name).into_iter().flatten())
        .filter(|item| item.lang == *default_lang)
        .collect();
    let rss = feed::generate_rss(config, &default_feed_items)?;
    fs::write(paths.output.join("feed.xml"), rss)?;
    let atom = feed::generate_atom(config, &default_feed_items, &format!("{base}/atom.xml"))?;
    fs::write(paths.output.join("atom.xml"), atom)?;

    // Per-language RSS and Atom feeds
    if is_multilingual {
        for lang in config.languages.keys() {
            let lang_feed_items: Vec<&ContentItem> = config
                .collections
                .iter()
                .filter(|c| c.has_rss && !c.private)
                .flat_map(|c| all_collections.get(&c.name).into_iter().flatten())
                .filter(|item| item.lang == *lang)
                .collect();
            if !lang_feed_items.is_empty() {
                let lang_dir = paths.output.join(lang);
                fs::create_dir_all(&lang_dir)?;
                let lang_rss = feed::generate_rss(config, &lang_feed_items)?;
                fs::write(lang_dir.join("feed.xml"), lang_rss)?;
                let lang_atom = feed::generate_atom(
                    config,
                    &lang_feed_items,
                    &format!("{base}/{lang}/atom.xml"),
                )?;
                fs::write(lang_dir.join("atom.xml"), lang_atom)?;
            }
        }
    }

    step_timings.push((
        "Generate feeds".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Private collections (e.g. behind Cloudflare Access) are kept out of every
    // public discovery surface below, even though their pages were fully built.
    let private_collections: std::collections::HashSet<&str> = config
        .collections
        .iter()
        .filter(|c| c.private)
        .map(|c| c.name.as_str())
        .collect();
    if !private_collections.is_empty() {
        let excluded: usize = all_collections
            .iter()
            .filter(|(name, _)| private_collections.contains(name.as_str()))
            .map(|(_, items)| items.len())
            .sum();
        if excluded > 0 {
            crate::output::human::info(&format!(
                "{excluded} private pages excluded from discovery"
            ));
        }
    }

    // Step 6: Generate sitemap (all non-private items, all languages)
    progress.step("Generating sitemap");
    let step_start = Instant::now();
    let all_items: Vec<&ContentItem> = all_collections
        .iter()
        .filter(|(name, _)| !private_collections.contains(name.as_str()))
        .flat_map(|(_, items)| items)
        .collect();
    let sitemap_xml =
        sitemap::generate_sitemap(config, &all_items, &translation_map, &tag_page_urls)?;
    fs::write(paths.output.join("sitemap.xml"), sitemap_xml)?;
    step_timings.push((
        "Generate sitemap".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 7: Generate discovery files (robots.txt, llms.txt, llms-full.txt)
    progress.step("Generating discovery files");
    let step_start = Instant::now();
    let robots = discovery::generate_robots_txt(config);
    fs::write(paths.output.join("robots.txt"), robots)?;

    // Default language discovery files
    let default_discovery_collections: Vec<(String, Vec<&ContentItem>)> = config
        .collections
        .iter()
        .filter(|c| !c.private)
        .map(|c| {
            let items: Vec<&ContentItem> = all_collections
                .get(&c.name)
                .into_iter()
                .flatten()
                .filter(|item| item.lang == *default_lang)
                .collect();
            (c.label.clone(), items)
        })
        .collect();
    let llms_txt = discovery::generate_llms_txt(config, &default_discovery_collections);
    fs::write(paths.output.join("llms.txt"), llms_txt)?;
    let llms_full = discovery::generate_llms_full_txt(config, &default_discovery_collections);
    fs::write(paths.output.join("llms-full.txt"), llms_full)?;

    // Per-language discovery files
    if is_multilingual {
        for lang in config.languages.keys() {
            let lang_collections: Vec<(String, Vec<&ContentItem>)> = config
                .collections
                .iter()
                .filter(|c| !c.private)
                .map(|c| {
                    let items: Vec<&ContentItem> = all_collections
                        .get(&c.name)
                        .into_iter()
                        .flatten()
                        .filter(|item| item.lang == *lang)
                        .collect();
                    (c.label.clone(), items)
                })
                .collect();
            let lang_dir = paths.output.join(lang);
            fs::create_dir_all(&lang_dir)?;
            let llms = discovery::generate_llms_txt(config, &lang_collections);
            fs::write(lang_dir.join("llms.txt"), llms)?;
            let llms_f = discovery::generate_llms_full_txt(config, &lang_collections);
            fs::write(lang_dir.join("llms-full.txt"), llms_f)?;
        }
    }

    step_timings.push((
        "Generate discovery files".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 8: Output raw markdown alongside HTML for each page
    progress.step("Writing markdown copies");
    let step_start = Instant::now();
    for collection in &config.collections {
        if let Some(items) = all_collections.get(&collection.name) {
            let md_results: Vec<(PathBuf, String)> = items
                .par_iter()
                .map(|item| {
                    let md_path = url_to_md_path(&paths.output, &item.url);
                    let md_content = format!(
                        "{}\n\n{}",
                        content::generate_frontmatter(&item.frontmatter),
                        item.raw_body
                    );
                    (md_path, md_content)
                })
                .collect();

            for (md_path, md_content) in md_results {
                if let Some(parent) = md_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&md_path, md_content)?;
            }
        }
    }

    step_timings.push((
        "Output markdown".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 9: Generate search index
    progress.step("Generating search index");
    let step_start = Instant::now();
    let all_search_items: Vec<&ContentItem> = all_collections.values().flatten().collect();

    // Root index: default language only
    let default_search_items: Vec<&ContentItem> = all_search_items
        .iter()
        .filter(|i| i.lang == *default_lang)
        .copied()
        .collect();
    let search_json = generate_search_index(&default_search_items, config);
    fs::write(paths.output.join("search-index.json"), &search_json)?;

    // Per-language indexes for non-default languages
    if is_multilingual {
        for lang_code in config.languages.keys() {
            let lang_items: Vec<&ContentItem> = all_search_items
                .iter()
                .filter(|i| i.lang == *lang_code)
                .copied()
                .collect();
            let lang_json = generate_search_index(&lang_items, config);
            let lang_dir = paths.output.join(lang_code);
            fs::create_dir_all(&lang_dir)?;
            fs::write(lang_dir.join("search-index.json"), lang_json)?;
        }
    }

    step_timings.push((
        "Generate search index".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 9b: Generate redirect pages from aliases
    let step_start = Instant::now();
    let all_redirect_items: Vec<&ContentItem> = all_collections.values().flatten().collect();
    let redirect_count = redirects::generate_redirects(config, &all_redirect_items, &paths.output)?;
    if redirect_count > 0 {
        step_timings.push((
            format!("Generate {redirect_count} redirects"),
            step_start.elapsed().as_secs_f64() * 1000.0,
        ));
    }

    // Step 10: Copy static files (with optional minification and fingerprinting)
    progress.step("Copying static files");
    let step_start = Instant::now();
    let mut static_count = 0;
    // manifest: maps "/static/foo.css" → "/static/foo.<hash8>.css" (only when fingerprinting)
    let mut asset_manifest: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let protected_asset_groups = config.password_access_groups();
    if paths.static_dir.exists() {
        for entry in WalkDir::new(&paths.static_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(&paths.static_dir)
                .unwrap_or(entry.path());
            let protected_rel = if config.access.is_some()
                && rel
                    .components()
                    .next()
                    .is_some_and(|part| part.as_os_str() == "private")
            {
                let mut parts = rel.components();
                parts.next();
                let Some(group) = parts.next().and_then(|part| part.as_os_str().to_str()) else {
                    continue;
                };
                if !protected_asset_groups.contains(&group) {
                    // A different output (for example, a private subdomain) may own
                    // this group. Never copy it into a public static directory.
                    continue;
                }
                let mut protected = PathBuf::from(group);
                protected.extend(parts.map(|part| part.as_os_str()));
                Some(protected)
            } else {
                None
            };
            let (dest, url_root, output_rel) = if let Some(protected) = protected_rel {
                (
                    paths.output.join("private-assets").join(&protected),
                    "/private-assets",
                    protected,
                )
            } else {
                (
                    paths.output.join("static").join(rel),
                    "/static",
                    rel.to_path_buf(),
                )
            };
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let is_css = ext == "css";
            let is_js = ext == "js";

            if (config.build.minify && (is_css || is_js)) || config.build.fingerprint {
                let content = fs::read(entry.path())?;
                let processed: Vec<u8> = if config.build.minify && is_css {
                    minify_css(&content).into_bytes()
                } else if config.build.minify && is_js {
                    minify_js(&content).into_bytes()
                } else {
                    content.clone()
                };

                fs::write(&dest, &processed)?;

                if config.build.fingerprint {
                    let hash = fnv_hash8(&processed);
                    // Build fingerprinted name: foo.css → foo.<hash>.css
                    let stem = entry
                        .path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let fp_name = format!("{stem}.{hash}.{ext}");
                    let fp_dest = dest
                        .parent()
                        .map(|p| p.join(&fp_name))
                        .unwrap_or_else(|| PathBuf::from(&fp_name));
                    fs::write(&fp_dest, &processed)?;

                    // Record in manifest using Unix-style paths
                    let orig_url = format!(
                        "{url_root}/{}",
                        output_rel.to_string_lossy().replace('\\', "/")
                    );
                    let fp_rel = output_rel
                        .parent()
                        .map(|p| {
                            let p = p.to_string_lossy();
                            if p.is_empty() {
                                fp_name.clone()
                            } else {
                                format!("{}/{fp_name}", p.replace('\\', "/"))
                            }
                        })
                        .unwrap_or_else(|| fp_name.clone());
                    let fp_url = format!("{url_root}/{fp_rel}");
                    asset_manifest.insert(orig_url, fp_url);
                }
            } else {
                fs::copy(entry.path(), &dest)?;
            }
            static_count += 1;
        }
    }

    // Write asset manifest if fingerprinting is on
    if config.build.fingerprint && !asset_manifest.is_empty() {
        let manifest_json =
            serde_json::to_string_pretty(&asset_manifest).unwrap_or_else(|_| "{}".to_string());
        fs::write(paths.output.join("asset-manifest.json"), manifest_json)?;
    }

    step_timings.push((
        "Copy static files".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 11: Process images (resize, WebP, srcset)
    progress.step("Processing images");
    let step_start = Instant::now();
    if config.access.is_some() {
        images::clear_private_static_output(paths)?;
    }
    let image_manifest = if let Some(ref images_config) = config.images {
        if !images_config.widths.is_empty() {
            images::process_images_with_private_filter(
                paths,
                images_config,
                config.access.is_some(),
            )?
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    step_timings.push((
        "Process images".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 12: Post-process all HTML files in a single pass
    // (image srcset, code copy buttons, subdomain link rewriting, base path rewriting, analytics)
    progress.step("Post-processing HTML");
    let step_start = Instant::now();
    let lazy_loading = config.images.as_ref().is_some_and(|img| img.lazy_loading);
    let needs_image_rewrite = !image_manifest.is_empty() || lazy_loading;
    let site_base_path = config.base_path();
    let computed_rewrites;
    let subdomain_rewrites = match subdomain_rewrites_override {
        Some(overrides) => overrides,
        None => {
            computed_rewrites = config.subdomain_rewrite_map();
            &computed_rewrites
        }
    };
    if needs_plausible_extensions_warning(config.analytics.as_ref()) {
        crate::output::human::warning(
            "Plausible `extensions` are deprecated (Oct 2025). Retrieve your unique snippet \
             from Plausible → Settings → Installation and set `script_url` in seite.toml. \
             Your current config still works. \
             See https://plausible.io/docs/script-update-guide",
        );
    }
    let post_ctx = HtmlPostProcessContext {
        image_manifest: &image_manifest,
        lazy_loading,
        needs_image_rewrite,
        subdomain_rewrites,
        base_path: &site_base_path,
        analytics: config.analytics.as_ref(),
        mermaid: config.build.mermaid,
    };
    let mut link_check = post_process_html_files(&paths.output, &post_ctx)?;
    link_check.broken_links.extend(md_link_problems);
    {
        // Report problems against the file they were written in (markdown
        // source, template, or data file) rather than the generated HTML.
        let output_display = links::relative_display(&paths.root, display_output);
        // Links not in a page's own markdown: templates and data (nav) first,
        // then other content files (post excerpts shown on listing pages).
        let fallback_dirs = [
            paths.templates.clone(),
            paths.data_dir.clone(),
            paths.content.clone(),
        ];
        let attribution = links::SourceAttribution {
            root: &paths.root,
            page_sources: &page_sources,
            fallback_dirs: &fallback_dirs,
            output_display: &output_display,
        };
        links::attribute_sources(&mut link_check.broken_links, &attribution);
        links::attribute_sources(&mut link_check.missing_assets, &attribution);
    }
    step_timings.push((
        "Post-process HTML".to_string(),
        step_start.elapsed().as_secs_f64() * 1000.0,
    ));

    // Step 13: Generate the Cloudflare Pages advanced-mode Worker for any
    // password-protected routes in this output.
    access::write_worker(config, &paths.output)?;

    progress.done();

    // Warn about public/ files overwritten by generated files
    let known_generated = [
        "robots.txt",
        "sitemap.xml",
        "feed.xml",
        "atom.xml",
        "_redirects",
        "llms.txt",
        "llms-full.txt",
        "search-index.json",
        "index.html",
        "404.html",
        "asset-manifest.json",
    ];
    for rel_path in &public_file_paths {
        if known_generated.contains(&rel_path.as_str()) {
            crate::output::human::warning(&format!(
                "public/{rel_path} was overwritten by the generated {rel_path} — remove it from public/ or configure the feature that generates it"
            ));
        }
    }

    let items_built: HashMap<String, usize> = all_collections
        .iter()
        .map(|(name, items)| (name.clone(), items.len()))
        .collect();

    // Calculate items skipped in incremental mode
    let items_skipped = if is_incremental {
        let total: usize = items_built.values().sum();
        let changed = changeset
            .as_ref()
            .map(|cs| cs.changed_content.len())
            .unwrap_or(0);
        total.saturating_sub(changed)
    } else {
        0
    };

    let data_files_count = crate::data::count_data_files(&paths.data_dir);
    let stats = BuildStats {
        items_built,
        static_files_copied: static_count,
        public_files_copied: public_count,
        data_files_loaded: data_files_count,
        duration_ms: start.elapsed().as_millis() as u64,
        step_timings,
        incremental: is_incremental,
        items_skipped,
    };

    Ok(BuildResult {
        collections: all_collections,
        stats,
        link_check,
        subdomain_builds: Vec::new(),
        warnings,
    })
}

/// All config needed for the unified HTML post-processing pass.
struct HtmlPostProcessContext<'a> {
    image_manifest: &'a HashMap<String, images::ProcessedImage>,
    lazy_loading: bool,
    needs_image_rewrite: bool,
    subdomain_rewrites: &'a HashMap<String, String>,
    base_path: &'a str,
    analytics: Option<&'a AnalyticsSection>,
    mermaid: bool,
}

/// Walk all `.html` files once, apply all post-processing transforms in memory, write once.
/// Also extracts internal links for validation, eliminating a separate file walk.
///
/// Consolidates image srcset rewriting, code copy button injection, base path
/// URL rewriting, analytics injection, and link extraction into a single pass.
fn post_process_html_files(
    output_dir: &Path,
    ctx: &HtmlPostProcessContext,
) -> Result<links::LinkCheckResult> {
    // Walk ALL files once to build valid URL set and collect HTML entries
    let all_files: Vec<_> = WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    let valid_urls = links::build_valid_urls_from_entries(output_dir, &all_files);

    let html_entries: Vec<_> = all_files
        .into_iter()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        .collect();

    // Per-file link check result or error
    let results: Vec<std::result::Result<links::PageRefCheck, PageError>> = html_entries
        .par_iter()
        .map(|entry| {
            let original = fs::read_to_string(entry.path()).map_err(PageError::from)?;

            let mut html = original.clone();

            // 1. Image srcset rewrite
            if ctx.needs_image_rewrite && html.contains("<img ") {
                html = images::rewrite_html_images(&html, ctx.image_manifest, ctx.lazy_loading);
            }

            // 2. Code copy button injection
            if html.contains("<pre") {
                html = code_copy::inject_code_copy(&html);
            }

            // 3. Cross-subdomain link rewriting
            if !ctx.subdomain_rewrites.is_empty() {
                html = links::rewrite_subdomain_links(&html, ctx.subdomain_rewrites);
            }

            // 4. Base path URL rewriting
            if !ctx.base_path.is_empty() {
                html = base_path::rewrite_html_urls(&html, ctx.base_path);
            }

            // 5. Analytics injection
            if let Some(analytics_config) = ctx.analytics {
                html = analytics::inject_analytics(&html, analytics_config);
            }

            // 6. Mermaid loader injection (no-op unless the page has a diagram)
            if ctx.mermaid {
                html = mermaid::inject_mermaid(&html);
            }

            // Only write if something changed
            if html != original {
                fs::write(entry.path(), &html).map_err(PageError::from)?;
            }

            // 7. Validate internal links and asset references in the final HTML
            let rel_path = entry
                .path()
                .strip_prefix(output_dir)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            Ok(links::check_page_refs(
                &html,
                &rel_path,
                &valid_urls,
                ctx.base_path,
            ))
        })
        .collect();

    let mut check = links::LinkCheckResult::default();
    for result in results {
        let page = result?;
        check.total_links_checked += page.checked;
        check.broken_links.extend(page.broken_links);
        check.missing_assets.extend(page.missing_assets);
    }
    links::fill_suggestions(&mut check.broken_links, &valid_urls);
    links::fill_suggestions(&mut check.missing_assets, &valid_urls);

    Ok(check)
}

/// Where a content file lands in the built site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLocation {
    pub slug: String,
    pub url: String,
    pub lang: String,
}

/// Resolve the slug, URL, and language of a content file exactly as the build
/// does (explicit `slug:` frontmatter, date-prefix stripping, nested
/// directories, and `.{lang}.md` translation suffixes).
///
/// `path` is the file path; `rel_path` is the same file relative to the
/// collection directory. Other tools (e.g. the MCP server) use this so the URLs
/// they report always match the generated output.
pub fn resolve_item_location(
    config: &SiteConfig,
    collection: &CollectionConfig,
    path: &Path,
    rel_path: &Path,
    fm: &Frontmatter,
) -> ItemLocation {
    let default_lang = &config.site.language;
    let configured_langs = config.configured_lang_codes();
    let file_lang = if config.is_multilingual() {
        content::extract_lang_from_filename(path, &configured_langs)
    } else {
        None
    };
    let lang = file_lang
        .as_deref()
        .unwrap_or(default_lang.as_str())
        .to_string();

    let slug = if file_lang.is_some() {
        resolve_slug_i18n(fm, rel_path, collection, &configured_langs)
    } else {
        resolve_slug(fm, rel_path, collection)
    };

    let base_url = build_url(&collection.url_prefix, &slug);
    let url = if lang != *default_lang {
        format!("/{lang}{base_url}")
    } else {
        base_url
    };
    ItemLocation { slug, url, lang }
}

/// Date for a content file: its `date:` frontmatter, or for dated collections
/// the `YYYY-MM-DD-` filename prefix (as the build does).
pub fn resolve_item_date(
    fm: &Frontmatter,
    path: &Path,
    collection: &CollectionConfig,
) -> Option<chrono::NaiveDate> {
    if fm.date.is_none() && collection.has_date {
        parse_date_from_filename(path)
    } else {
        fm.date
    }
}

fn resolve_slug(fm: &Frontmatter, rel_path: &Path, collection: &CollectionConfig) -> String {
    if let Some(ref s) = fm.slug {
        return s.clone();
    }

    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");

    let filename_slug = if collection.has_date
        && stem.len() > 11
        && stem.as_bytes()[4] == b'-'
        && stem.as_bytes()[7] == b'-'
        && stem.as_bytes()[10] == b'-'
    {
        &stem[11..]
    } else {
        stem
    };

    if collection.nested {
        if let Some(parent) = rel_path.parent() {
            let parent_str = parent.to_str().unwrap_or("");
            if !parent_str.is_empty() {
                // Normalize backslashes for Windows paths → URL-style forward slashes
                let parent_str = parent_str.replace('\\', "/");
                return format!("{parent_str}/{filename_slug}");
            }
        }
    }

    filename_slug.to_string()
}

/// Resolve slug for a translated file by stripping the language suffix from
/// the filename before delegating to `resolve_slug`.
fn resolve_slug_i18n(
    fm: &Frontmatter,
    rel_path: &Path,
    collection: &CollectionConfig,
    configured_langs: &std::collections::HashSet<&str>,
) -> String {
    // If frontmatter has an explicit slug, use it directly
    if fm.slug.is_some() {
        return resolve_slug(fm, rel_path, collection);
    }
    // Strip lang suffix from the stem: "about.es" → "about"
    let stem = rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    let stripped = content::strip_lang_suffix(stem, configured_langs);
    let new_filename = format!("{stripped}.md");
    let new_rel = if let Some(parent) = rel_path.parent() {
        if parent.as_os_str().is_empty() {
            PathBuf::from(&new_filename)
        } else {
            parent.join(&new_filename)
        }
    } else {
        PathBuf::from(&new_filename)
    };
    resolve_slug(fm, &new_rel, collection)
}

fn parse_date_from_filename(path: &Path) -> Option<chrono::NaiveDate> {
    let stem = path.file_stem()?.to_str()?;
    if stem.len() >= 10 {
        match chrono::NaiveDate::parse_from_str(&stem[..10], "%Y-%m-%d") {
            Ok(date) => Some(date),
            Err(e) => {
                tracing::warn!("Malformed date in filename '{}': {e}", path.display());
                None
            }
        }
    } else {
        None
    }
}

fn build_url(url_prefix: &str, slug: &str) -> String {
    let prefix = url_prefix.trim_end_matches('/');
    if prefix.is_empty() {
        format!("/{slug}")
    } else {
        format!("{prefix}/{slug}")
    }
}

/// FNV-1a hash → first 8 hex chars, used for cache-busting fingerprints.
fn fnv_hash8(data: &[u8]) -> String {
    let mut hash: u64 = 14695981039346656037;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")[..8].to_string()
}

/// Minify CSS: strip comments and collapse whitespace around syntax characters.
fn minify_css(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    // Remove block comments
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next(); // consume '*'
                          // Skip until '*/'
            while let Some(c2) = chars.next() {
                if c2 == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    // Collapse runs of whitespace (including newlines) into a single space
    let mut result = String::with_capacity(out.len());
    let mut prev_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            result.push(c);
        }
    }
    // Remove spaces around CSS delimiters
    for (pat, rep) in &[
        (" { ", "{"),
        ("{ ", "{"),
        (" {", "{"),
        (" } ", "}"),
        ("} ", "}"),
        (" }", "}"),
        (" : ", ":"),
        (": ", ":"),
        (" :", ":"),
        (" ; ", ";"),
        ("; ", ";"),
        (" ;", ";"),
        (" , ", ","),
        (", ", ","),
        (" ,", ","),
    ] {
        result = result.replace(pat, rep);
    }
    result.trim().to_string()
}

/// Minify JS: strip line comments and collapse blank lines.
/// Deliberately conservative — does not touch string contents or block structure.
fn minify_js(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut in_single = false; // inside single-line string ''
    let mut in_double = false; // inside double-line string ""
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Toggle string state (simplified — doesn't handle escape sequences perfectly)
        if c == '\'' && !in_double {
            in_single = !in_single;
        }
        if c == '"' && !in_single {
            in_double = !in_double;
        }
        // Strip // line comments only outside strings
        if !in_single && !in_double && c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push('\n');
            continue;
        }
        out.push(c);
        i += 1;
    }
    // Remove blank lines
    let result: String = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    result
}

/// Output HTML path of a page URL, relative to the output directory
/// (`/posts/hello` → `posts/hello.html`, `/docs/index` → `docs/index.html`).
fn output_rel_html(url: &str) -> String {
    format!("{}.html", url.trim_matches('/'))
}

/// Rewrite links to markdown source files in every item's HTML body and
/// excerpt (see [`markdown::rewrite_md_links`]).
///
/// Returns a map of output-relative HTML path → source file for every item
/// (used to attribute broken links to their markdown source), plus a
/// `Markdown`-kind broken link for each relative `.md` link that matched no
/// content file, located at its line in the source file.
fn rewrite_source_links(
    all_collections: &mut HashMap<String, Vec<ContentItem>>,
    paths: &ResolvedPaths,
    default_lang: &str,
) -> (HashMap<String, PathBuf>, Vec<links::BrokenLink>) {
    let mut map = markdown::SourceLinkMap::new(&paths.root, &paths.content, default_lang);
    let mut page_sources = HashMap::new();
    for items in all_collections.values() {
        for item in items {
            map.insert(&item.source_path, &item.url);
            page_sources.insert(output_rel_html(&item.url), item.source_path.clone());
        }
    }

    let mut problems = Vec::new();
    for items in all_collections.values_mut() {
        let per_item: Vec<Vec<links::BrokenLink>> = items
            .par_iter_mut()
            .map(|item| {
                if !item.html_body.contains(".md") && !item.excerpt_html.contains(".md") {
                    return Vec::new();
                }
                let (html, unresolved) = markdown::rewrite_md_links(
                    &item.html_body,
                    &item.source_path,
                    &item.lang,
                    &map,
                );
                item.html_body = html;
                let (excerpt, _) = markdown::rewrite_md_links(
                    &item.excerpt_html,
                    &item.source_path,
                    &item.lang,
                    &map,
                );
                item.excerpt_html = excerpt;
                if unresolved.is_empty() {
                    return Vec::new();
                }
                let text = fs::read_to_string(&item.source_path).ok();
                let source = links::relative_display(&paths.root, &item.source_path);
                let page = output_rel_html(&item.url);
                unresolved
                    .into_iter()
                    .map(|href| {
                        let mut link =
                            links::BrokenLink::new(page.clone(), href, links::LinkKind::Markdown);
                        link.source = Some(source.clone());
                        link.line = text
                            .as_deref()
                            .and_then(|t| links::find_link_line(t, &link.href));
                        link
                    })
                    .collect()
            })
            .collect();
        problems.extend(per_item.into_iter().flatten());
    }
    (page_sources, problems)
}

fn url_to_output_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.html"))
}

fn url_to_md_path(output_dir: &Path, url: &str) -> std::path::PathBuf {
    let clean = url.trim_matches('/');
    output_dir.join(format!("{clean}.md"))
}

/// Generate a markdown listing for a collection index page.
fn generate_collection_index_md(label: &str, items: &[ItemSummary]) -> String {
    let mut md = format!("# {label}\n\n");
    for item in items {
        md.push_str(&format!("- [{}]({})", item.title, item.url));
        if let Some(date) = &item.date {
            md.push_str(&format!(" ({date})"));
        }
        md.push('\n');
        if let Some(desc) = &item.description {
            if !desc.is_empty() {
                md.push_str(&format!("  {desc}\n"));
            }
        }
    }
    md
}

/// Generate an HTML redirect page (meta refresh + JS redirect).
fn generate_redirect_html(target_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0; url={target_url}">
<link rel="canonical" href="{target_url}">
<title>Redirecting…</title>
<script>window.location.replace("{target_url}");</script>
</head>
<body>
<p>Redirecting to <a href="{target_url}">{target_url}</a>…</p>
</body>
</html>"#
    )
}

/// Robots directive for a page: the page's own `robots` frontmatter if set,
/// otherwise `noindex, nofollow` when its collection is `private`, else `None`.
fn effective_robots(own: Option<String>, collection_private: bool) -> Option<String> {
    own.or_else(|| collection_private.then(|| "noindex, nofollow".to_string()))
}

fn build_page_context(
    site: &SiteContext,
    item: &ContentItem,
    data: &serde_json::Value,
    collection_private: bool,
) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("site", site);
    ctx.insert("data", data);
    ctx.insert(
        "page",
        &PageContext {
            title: item.frontmatter.title.clone(),
            content: item.html_body.clone(),
            date: item.frontmatter.date.map(|d| d.to_string()),
            updated: item.frontmatter.updated.map(|d| d.to_string()),
            description: item.frontmatter.description.clone(),
            image: absolutize_image(item.frontmatter.image.as_deref(), &site.base_url),
            slug: item.slug.clone(),
            tags: item.frontmatter.tags.clone(),
            url: item.url.clone(),
            collection: item.collection.clone(),
            robots: effective_robots(item.frontmatter.robots.clone(), collection_private),
            word_count: item.word_count,
            reading_time: item.reading_time,
            excerpt: item.excerpt_html.clone(),
            toc: item.toc.clone(),
            extra: item.frontmatter.extra.clone(),
        },
    );
    ctx
}

/// Ensure an image URL is absolute by prepending `base_url` if it's a relative path.
/// Absolute URLs (starting with `http`) are returned unchanged.
fn absolutize_image(image: Option<&str>, base_url: &str) -> Option<String> {
    image.map(|img| {
        if img.starts_with("http") {
            img.to_string()
        } else {
            format!("{base_url}{img}")
        }
    })
}

/// Compute the language URL prefix: empty for the default language, `"/{lang}"` for others.
fn lang_prefix_for(lang: &str, default_lang: &str) -> String {
    if lang == default_lang {
        String::new()
    } else {
        format!("/{lang}")
    }
}

/// Return a JSON object of common UI strings for a given language.
///
/// Starts with English defaults, then merges any overrides from
/// `data.i18n.{lang}` (i.e. `data/i18n/{lang}.yaml`).
fn ui_strings_for_lang(lang: &str, data: &serde_json::Value) -> serde_json::Value {
    let defaults = serde_json::json!({
        "search_placeholder": "Search\u{2026}",
        "skip_to_content": "Skip to main content",
        "no_results": "No results",
        "newer": "Newer",
        "older": "Older",
        "page_n_of_total": "Page {n} of {total}",
        "search_label": "Search site content",
        "min_read": "min read",
        "contents": "Contents",
        "tags": "Tags",
        "all_tags": "All tags",
        "tagged": "Tagged",
        "changelog": "Changelog",
        "all_releases": "All releases",
        "roadmap": "Roadmap",
        "not_found_title": "Page Not Found",
        "not_found_message": "The page you requested could not be found.",
        "go_home": "Go to the homepage",
        "in_progress": "In Progress",
        "planned": "Planned",
        "done": "Done",
        "other": "Other",
        "trust_center": "Trust Center",
        "trust_hero_subtitle": "Security, compliance, and data protection at {site}.",
        "certifications_compliance": "Certifications & Compliance",
        "active": "Active",
        "learn_more": "Learn more",
        "auditor": "Auditor",
        "scope": "Scope",
        "issued": "Issued",
        "expires": "Expires",
        "subprocessors": "Subprocessors",
        "vendor": "Vendor",
        "purpose": "Purpose",
        "location": "Location",
        "dpa": "DPA",
        "yes": "Yes",
        "no": "No",
        "faq": "Frequently Asked Questions",
        "resources": "Resources",
        "previous": "Previous",
        "next": "Next",
        "prev_post": "Previous",
        "next_post": "Next",
        "on_this_page": "On this page",
        "search_docs": "Search docs\u{2026}",
        "search_documentation": "Search documentation",
        "toggle_theme": "Toggle light/dark mode",
        "toggle_sidebar": "Toggle sidebar",
        "built_with": "Built with",
        "get_started": "Get started",
        "view_on_github": "View on GitHub",
        "rss": "RSS",
        "changelog_subtitle": "All notable changes. Subscribe via",
        "roadmap_subtitle": "What we're working on and what's coming next.",
        "open_an_issue": "Open an issue",
        "have_a_feature_request": "Have a feature request?",
        "contact_name": "Name",
        "contact_email": "Email",
        "contact_message": "Message",
        "contact_submit": "Send Message",
        "documentation": "Documentation",
        "callout_info": "Info",
        "callout_tip": "Tip",
        "callout_warning": "Warning",
        "callout_danger": "Danger"
    });

    // Check data.i18n.{lang} for overrides, merge on top of defaults
    if let Some(i18n) = data
        .get("i18n")
        .and_then(|i| i.get(lang))
        .and_then(|v| v.as_object())
    {
        let mut merged = defaults
            .as_object()
            .expect("UI string defaults must be a JSON object")
            .clone();
        for (k, v) in i18n {
            merged.insert(k.clone(), v.clone());
        }
        serde_json::Value::Object(merged)
    } else {
        defaults
    }
}

/// Insert `lang_prefix`, `default_language`, and `t` into a Tera context.
fn insert_i18n_context(
    ctx: &mut tera::Context,
    lang: &str,
    default_lang: &str,
    data: &serde_json::Value,
) {
    ctx.insert("lang_prefix", &lang_prefix_for(lang, default_lang));
    ctx.insert("default_language", default_lang);
    ctx.insert("t", &ui_strings_for_lang(lang, data));
}

/// Insert pre-cached i18n context into a Tera context. Avoids rebuilding the
/// 37-key UI strings JSON object on every page render.
fn insert_i18n_context_cached(
    ctx: &mut tera::Context,
    cached: &(String, String, serde_json::Value),
) {
    ctx.insert("lang_prefix", &cached.0);
    ctx.insert("default_language", &cached.1);
    ctx.insert("t", &cached.2);
}

/// Insert build feature flags into template context.
fn insert_build_flags(ctx: &mut tera::Context, config: &SiteConfig) {
    ctx.insert("math_enabled", &config.build.math);
    if config.build.math {
        ctx.insert("katex_css_url", math::KATEX_CSS_URL);
    }
}

/// Build a JSON search index from a slice of content items.
/// Only items from `listed: true` collections are included.
fn generate_search_index(items: &[&ContentItem], config: &SiteConfig) -> String {
    // Listed, non-private collections only: private content stays out of search.
    let listed: std::collections::HashSet<&str> = config
        .collections
        .iter()
        .filter(|c| c.listed && !c.private)
        .map(|c| c.name.as_str())
        .collect();

    let entries: Vec<SearchEntry> = items
        .iter()
        .filter(|item| listed.contains(item.collection.as_str()))
        .map(|item| SearchEntry {
            title: &item.frontmatter.title,
            description: item.frontmatter.description.as_deref(),
            url: &item.url,
            collection: &item.collection,
            tags: &item.frontmatter.tags,
            date: item.frontmatter.date.map(|d| d.to_string()),
            lang: &item.lang,
        })
        .collect();

    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
}

/// Convert a slug segment to title case (e.g., "getting-started" → "Getting Started").
fn title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AccessSection, BuildSection, CollectionConfig, DeploySection, LanguageConfig, SiteConfig,
        SiteSection,
    };
    use crate::content::Frontmatter;
    use std::collections::{BTreeMap, HashMap, HashSet};
    use std::path::{Path, PathBuf};

    #[test]
    fn test_effective_robots() {
        // A page's own robots frontmatter always wins, private or not.
        assert_eq!(
            effective_robots(Some("noindex".into()), false),
            Some("noindex".into())
        );
        assert_eq!(
            effective_robots(Some("all".into()), true),
            Some("all".into())
        );
        // Private pages without their own robots get noindex, nofollow.
        assert_eq!(
            effective_robots(None, true),
            Some("noindex, nofollow".into())
        );
        // Public pages without their own robots get nothing.
        assert_eq!(effective_robots(None, false), None);
    }

    // ── Helper: minimal SiteConfig ──────────────────────────────────────
    fn minimal_config() -> SiteConfig {
        SiteConfig {
            site: SiteSection {
                title: "Test Site".into(),
                description: "A test".into(),
                base_url: "https://example.com".into(),
                language: "en".into(),
                author: "Author".into(),
            },
            collections: vec![CollectionConfig::preset_posts()],
            build: BuildSection::default(),
            deploy: DeploySection::default(),
            languages: BTreeMap::new(),
            images: None,
            analytics: None,
            trust: None,
            contact: None,
            access: None,
        }
    }

    fn multilingual_config() -> SiteConfig {
        let mut cfg = minimal_config();
        cfg.languages.insert(
            "es".into(),
            LanguageConfig {
                title: Some("Sitio de Prueba".into()),
                description: Some("Una prueba".into()),
            },
        );
        cfg.languages.insert(
            "fr".into(),
            LanguageConfig {
                title: None,
                description: None,
            },
        );
        cfg
    }

    fn default_frontmatter() -> Frontmatter {
        Frontmatter {
            title: "Test".into(),
            ..Frontmatter::default()
        }
    }

    // ── resolve_slug ────────────────────────────────────────────────────

    #[test]
    fn test_resolve_slug_explicit_slug_overrides() {
        let mut fm = default_frontmatter();
        fm.slug = Some("custom-slug".into());
        let coll = CollectionConfig::preset_posts();
        let slug = resolve_slug(&fm, Path::new("anything.md"), &coll);
        assert_eq!(slug, "custom-slug");
    }

    #[test]
    fn test_resolve_slug_simple_filename() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let slug = resolve_slug(&fm, Path::new("about.md"), &coll);
        assert_eq!(slug, "about");
    }

    #[test]
    fn test_resolve_slug_strips_date_prefix_for_date_collections() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_posts();
        let slug = resolve_slug(&fm, Path::new("2025-01-15-hello-world.md"), &coll);
        assert_eq!(slug, "hello-world");
    }

    #[test]
    fn test_resolve_slug_no_date_strip_for_non_date_collections() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let slug = resolve_slug(&fm, Path::new("2025-01-15-hello-world.md"), &coll);
        assert_eq!(slug, "2025-01-15-hello-world");
    }

    #[test]
    fn test_resolve_slug_nested_collection() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_docs();
        let slug = resolve_slug(&fm, Path::new("guides/setup.md"), &coll);
        assert_eq!(slug, "guides/setup");
    }

    #[test]
    fn test_resolve_slug_nested_deep_path() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_docs();
        let slug = resolve_slug(&fm, Path::new("guides/advanced/config.md"), &coll);
        assert_eq!(slug, "guides/advanced/config");
    }

    #[test]
    fn test_resolve_slug_nested_root_file() {
        // A file at the root of a nested collection (no parent dir)
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_docs();
        let slug = resolve_slug(&fm, Path::new("overview.md"), &coll);
        assert_eq!(slug, "overview");
    }

    #[test]
    fn test_resolve_slug_short_date_prefix_not_stripped() {
        // Filename that looks like a date but is too short
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_posts();
        let slug = resolve_slug(&fm, Path::new("2025-01.md"), &coll);
        assert_eq!(slug, "2025-01");
    }

    // ── resolve_slug_i18n ───────────────────────────────────────────────

    #[test]
    fn test_resolve_slug_i18n_strips_lang_suffix() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let langs: HashSet<&str> = ["es", "fr"].into_iter().collect();
        let slug = resolve_slug_i18n(&fm, Path::new("about.es.md"), &coll, &langs);
        assert_eq!(slug, "about");
    }

    #[test]
    fn test_resolve_slug_i18n_explicit_slug_passthrough() {
        let mut fm = default_frontmatter();
        fm.slug = Some("my-about".into());
        let coll = CollectionConfig::preset_pages();
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        let slug = resolve_slug_i18n(&fm, Path::new("about.es.md"), &coll, &langs);
        assert_eq!(slug, "my-about");
    }

    #[test]
    fn test_resolve_slug_i18n_no_lang_suffix() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        // No lang suffix — should work like regular slug
        let slug = resolve_slug_i18n(&fm, Path::new("about.md"), &coll, &langs);
        assert_eq!(slug, "about");
    }

    #[test]
    fn test_resolve_slug_i18n_with_parent_dir() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_docs(); // nested
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        let slug = resolve_slug_i18n(&fm, Path::new("guides/setup.es.md"), &coll, &langs);
        assert_eq!(slug, "guides/setup");
    }

    #[test]
    fn test_resolve_slug_i18n_date_collection_with_lang() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_posts();
        let langs: HashSet<&str> = ["fr"].into_iter().collect();
        let slug = resolve_slug_i18n(&fm, Path::new("2025-01-15-hello.fr.md"), &coll, &langs);
        assert_eq!(slug, "hello");
    }

    // ── parse_date_from_filename ────────────────────────────────────────

    #[test]
    fn test_parse_date_from_filename_valid() {
        let date = parse_date_from_filename(Path::new("2025-01-15-hello.md"));
        assert_eq!(
            date,
            Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap())
        );
    }

    #[test]
    fn test_parse_date_from_filename_short_stem() {
        let date = parse_date_from_filename(Path::new("hello.md"));
        assert_eq!(date, None);
    }

    #[test]
    fn test_parse_date_from_filename_malformed_date() {
        let date = parse_date_from_filename(Path::new("2025-99-99-hello.md"));
        assert_eq!(date, None);
    }

    #[test]
    fn test_parse_date_from_filename_exact_ten_chars() {
        let date = parse_date_from_filename(Path::new("2025-03-01.md"));
        assert_eq!(
            date,
            Some(chrono::NaiveDate::from_ymd_opt(2025, 3, 1).unwrap())
        );
    }

    #[test]
    fn test_parse_date_from_filename_no_extension() {
        // No file extension means file_stem returns the whole name
        let date = parse_date_from_filename(Path::new("2025-03-01"));
        assert_eq!(
            date,
            Some(chrono::NaiveDate::from_ymd_opt(2025, 3, 1).unwrap())
        );
    }

    // ── build_url ───────────────────────────────────────────────────────

    #[test]
    fn test_build_url_with_prefix() {
        assert_eq!(build_url("/posts", "hello-world"), "/posts/hello-world");
    }

    #[test]
    fn test_build_url_empty_prefix() {
        assert_eq!(build_url("", "about"), "/about");
    }

    #[test]
    fn test_build_url_trailing_slash_on_prefix() {
        assert_eq!(build_url("/posts/", "my-post"), "/posts/my-post");
    }

    // ── fnv_hash8 ───────────────────────────────────────────────────────

    #[test]
    fn test_fnv_hash8_deterministic() {
        let h1 = fnv_hash8(b"hello world");
        let h2 = fnv_hash8(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);
    }

    #[test]
    fn test_fnv_hash8_different_inputs_differ() {
        let h1 = fnv_hash8(b"hello");
        let h2 = fnv_hash8(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_fnv_hash8_empty_input() {
        let h = fnv_hash8(b"");
        assert_eq!(h.len(), 8);
        // FNV-1a with no bytes should return first 8 hex chars of the offset basis
        assert_eq!(h, "cbf29ce4");
    }

    // ── minify_css ──────────────────────────────────────────────────────

    #[test]
    fn test_minify_css_removes_comments() {
        let input = b"body { /* a comment */ color: red; }";
        let out = minify_css(input);
        assert!(!out.contains("comment"));
        assert!(out.contains("color:red"));
    }

    #[test]
    fn test_minify_css_collapses_whitespace() {
        let input = b"body  {  color :  red ;  }";
        let out = minify_css(input);
        assert_eq!(out, "body{color:red;}");
    }

    #[test]
    fn test_minify_css_handles_empty_input() {
        let out = minify_css(b"");
        assert_eq!(out, "");
    }

    #[test]
    fn test_minify_css_multiple_rules() {
        let input = b"h1 { font-size: 2rem; }\n\np { margin: 0; }";
        let out = minify_css(input);
        assert!(out.contains("h1{font-size:2rem;}"));
        assert!(out.contains("p{margin:0;}"));
    }

    #[test]
    fn test_minify_css_nested_comment() {
        let input = b"a { /* start /* nested */ color: blue; }";
        let out = minify_css(input);
        assert!(out.contains("color:blue"));
    }

    // ── minify_js ───────────────────────────────────────────────────────

    #[test]
    fn test_minify_js_removes_line_comments() {
        let input = b"var x = 1; // this is a comment\nvar y = 2;";
        let out = minify_js(input);
        assert!(!out.contains("this is a comment"));
        assert!(out.contains("var x = 1;"));
        assert!(out.contains("var y = 2;"));
    }

    #[test]
    fn test_minify_js_preserves_comments_in_strings() {
        let input = b"var url = \"http://example.com\";";
        let out = minify_js(input);
        assert!(out.contains("http://example.com"));
    }

    #[test]
    fn test_minify_js_removes_blank_lines() {
        let input = b"var x = 1;\n\n\nvar y = 2;";
        let out = minify_js(input);
        assert_eq!(out, "var x = 1;\nvar y = 2;");
    }

    #[test]
    fn test_minify_js_preserves_single_quoted_string_with_slashes() {
        let input = b"var s = '// not a comment';";
        let out = minify_js(input);
        assert!(out.contains("// not a comment"));
    }

    #[test]
    fn test_minify_js_empty_input() {
        let out = minify_js(b"");
        assert_eq!(out, "");
    }

    // ── url_to_output_path / url_to_md_path ─────────────────────────────

    #[test]
    fn test_url_to_output_path_basic() {
        let p = url_to_output_path(Path::new("/out"), "/posts/hello");
        assert_eq!(p, PathBuf::from("/out/posts/hello.html"));
    }

    #[test]
    fn test_url_to_output_path_strips_slashes() {
        let p = url_to_output_path(Path::new("/out"), "/about/");
        assert_eq!(p, PathBuf::from("/out/about.html"));
    }

    #[test]
    fn test_url_to_md_path_basic() {
        let p = url_to_md_path(Path::new("/out"), "/docs/setup");
        assert_eq!(p, PathBuf::from("/out/docs/setup.md"));
    }

    #[test]
    fn test_url_to_md_path_strips_slashes() {
        let p = url_to_md_path(Path::new("/out"), "/about/");
        assert_eq!(p, PathBuf::from("/out/about.md"));
    }

    // ── generate_collection_index_md ────────────────────────────────────

    #[test]
    fn test_generate_collection_index_md_basic() {
        let items = vec![ItemSummary {
            title: "Hello World".into(),
            date: Some("2025-01-15".into()),
            description: Some("A first post".into()),
            slug: "hello-world".into(),
            tags: vec!["rust".into()],
            url: "/posts/hello-world".into(),
            word_count: 100,
            reading_time: 1,
            excerpt: String::new(),
        }];
        let md = generate_collection_index_md("Posts", &items);
        assert!(md.starts_with("# Posts\n\n"));
        assert!(md.contains("[Hello World](/posts/hello-world)"));
        assert!(md.contains("(2025-01-15)"));
        assert!(md.contains("  A first post\n"));
    }

    #[test]
    fn test_generate_collection_index_md_no_date_no_description() {
        let items = vec![ItemSummary {
            title: "About".into(),
            date: None,
            description: None,
            slug: "about".into(),
            tags: vec![],
            url: "/about".into(),
            word_count: 50,
            reading_time: 1,
            excerpt: String::new(),
        }];
        let md = generate_collection_index_md("Pages", &items);
        assert!(md.contains("- [About](/about)\n"));
        // No date parenthetical should appear after the link
        assert!(!md.contains("/about) ("));
    }

    #[test]
    fn test_generate_collection_index_md_empty_items() {
        let md = generate_collection_index_md("Empty", &[]);
        assert_eq!(md, "# Empty\n\n");
    }

    #[test]
    fn test_generate_collection_index_md_empty_description_skipped() {
        let items = vec![ItemSummary {
            title: "Item".into(),
            date: None,
            description: Some(String::new()),
            slug: "item".into(),
            tags: vec![],
            url: "/item".into(),
            word_count: 10,
            reading_time: 1,
            excerpt: String::new(),
        }];
        let md = generate_collection_index_md("Stuff", &items);
        // Should NOT contain an indented empty description line
        assert_eq!(md, "# Stuff\n\n- [Item](/item)\n");
    }

    // ── absolutize_image ────────────────────────────────────────────────

    #[test]
    fn test_absolutize_image_none() {
        assert_eq!(absolutize_image(None, "https://example.com"), None);
    }

    #[test]
    fn test_absolutize_image_relative_path() {
        let result = absolutize_image(Some("/static/hero.png"), "https://example.com");
        assert_eq!(result, Some("https://example.com/static/hero.png".into()));
    }

    #[test]
    fn test_absolutize_image_absolute_url() {
        let result = absolutize_image(
            Some("https://cdn.example.com/img.png"),
            "https://example.com",
        );
        assert_eq!(result, Some("https://cdn.example.com/img.png".into()));
    }

    #[test]
    fn test_absolutize_image_http_url() {
        let result = absolutize_image(
            Some("http://cdn.example.com/img.png"),
            "https://example.com",
        );
        assert_eq!(result, Some("http://cdn.example.com/img.png".into()));
    }

    // ── lang_prefix_for ─────────────────────────────────────────────────

    #[test]
    fn test_lang_prefix_for_default_language() {
        assert_eq!(lang_prefix_for("en", "en"), "");
    }

    #[test]
    fn test_lang_prefix_for_non_default_language() {
        assert_eq!(lang_prefix_for("es", "en"), "/es");
    }

    #[test]
    fn test_lang_prefix_for_another_language() {
        assert_eq!(lang_prefix_for("fr", "en"), "/fr");
    }

    // ── ui_strings_for_lang ─────────────────────────────────────────────

    #[test]
    fn test_ui_strings_for_lang_defaults() {
        let data = serde_json::json!({});
        let t = ui_strings_for_lang("en", &data);
        assert_eq!(
            t.get("search_placeholder").unwrap().as_str().unwrap(),
            "Search\u{2026}"
        );
        assert_eq!(
            t.get("skip_to_content").unwrap().as_str().unwrap(),
            "Skip to main content"
        );
        assert_eq!(t.get("newer").unwrap().as_str().unwrap(), "Newer");
        assert_eq!(t.get("older").unwrap().as_str().unwrap(), "Older");
        assert_eq!(
            t.get("not_found_title").unwrap().as_str().unwrap(),
            "Page Not Found"
        );
        assert_eq!(t.get("contact_name").unwrap().as_str().unwrap(), "Name");
        assert_eq!(
            t.get("contact_submit").unwrap().as_str().unwrap(),
            "Send Message"
        );
    }

    #[test]
    fn test_ui_strings_for_lang_with_overrides() {
        let data = serde_json::json!({
            "i18n": {
                "es": {
                    "search_placeholder": "Buscar\u{2026}",
                    "newer": "M\u{00e1}s nuevo",
                    "custom_key": "Custom Value"
                }
            }
        });
        let t = ui_strings_for_lang("es", &data);
        // Overridden values
        assert_eq!(
            t.get("search_placeholder").unwrap().as_str().unwrap(),
            "Buscar\u{2026}"
        );
        assert_eq!(
            t.get("newer").unwrap().as_str().unwrap(),
            "M\u{00e1}s nuevo"
        );
        // Custom key added
        assert_eq!(
            t.get("custom_key").unwrap().as_str().unwrap(),
            "Custom Value"
        );
        // Non-overridden defaults still present
        assert_eq!(t.get("older").unwrap().as_str().unwrap(), "Older");
    }

    #[test]
    fn test_ui_strings_for_lang_no_matching_lang() {
        let data = serde_json::json!({
            "i18n": {
                "fr": { "newer": "Plus r\u{00e9}cent" }
            }
        });
        // Requesting "de" which has no overrides
        let t = ui_strings_for_lang("de", &data);
        assert_eq!(t.get("newer").unwrap().as_str().unwrap(), "Newer");
    }

    #[test]
    fn test_ui_strings_for_lang_i18n_not_object() {
        let data = serde_json::json!({
            "i18n": {
                "es": "not-an-object"
            }
        });
        let t = ui_strings_for_lang("es", &data);
        // Should fall back to defaults since value isn't an object
        assert_eq!(t.get("newer").unwrap().as_str().unwrap(), "Newer");
    }

    // ── insert_i18n_context ─────────────────────────────────────────────

    #[test]
    fn test_insert_i18n_context_default_lang() {
        let data = serde_json::json!({});
        let mut ctx = tera::Context::new();
        insert_i18n_context(&mut ctx, "en", "en", &data);
        let json = ctx.into_json();
        assert_eq!(json.get("lang_prefix").unwrap().as_str().unwrap(), "");
        assert_eq!(
            json.get("default_language").unwrap().as_str().unwrap(),
            "en"
        );
        assert!(json.get("t").unwrap().is_object());
    }

    #[test]
    fn test_insert_i18n_context_non_default_lang() {
        let data = serde_json::json!({});
        let mut ctx = tera::Context::new();
        insert_i18n_context(&mut ctx, "es", "en", &data);
        let json = ctx.into_json();
        assert_eq!(json.get("lang_prefix").unwrap().as_str().unwrap(), "/es");
        assert_eq!(
            json.get("default_language").unwrap().as_str().unwrap(),
            "en"
        );
    }

    // ── title_case ──────────────────────────────────────────────────────

    #[test]
    fn test_title_case_single_word() {
        assert_eq!(title_case("guides"), "Guides");
    }

    #[test]
    fn test_title_case_hyphenated() {
        assert_eq!(title_case("getting-started"), "Getting Started");
    }

    #[test]
    fn test_title_case_empty() {
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn test_title_case_already_capitalized() {
        assert_eq!(title_case("API"), "API");
    }

    #[test]
    fn test_title_case_multiple_hyphens() {
        assert_eq!(title_case("a-b-c-d"), "A B C D");
    }

    // ── generate_search_index ───────────────────────────────────────────

    #[test]
    fn test_generate_search_index_includes_listed_collections() {
        let config = minimal_config(); // posts is listed=true
        let item = ContentItem {
            frontmatter: Frontmatter {
                title: "Hello".into(),
                description: Some("A post".into()),
                date: Some(chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
                tags: vec!["rust".into()],
                ..Frontmatter::default()
            },
            raw_body: "body".into(),
            html_body: "<p>body</p>".into(),
            source_path: PathBuf::from("content/posts/hello.md"),
            slug: "hello".into(),
            collection: "posts".into(),
            url: "/posts/hello".into(),
            lang: "en".into(),
            excerpt: "body".into(),
            toc: vec![],
            word_count: 1,
            reading_time: 1,
            excerpt_html: "<p>body</p>".into(),
        };
        let items = vec![&item];
        let json = generate_search_index(&items, &config);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["title"], "Hello");
        assert_eq!(parsed[0]["url"], "/posts/hello");
        assert_eq!(parsed[0]["collection"], "posts");
    }

    #[test]
    fn test_generate_search_index_excludes_unlisted_collections() {
        let mut config = minimal_config();
        config.collections = vec![CollectionConfig::preset_pages()]; // listed=false

        let item = ContentItem {
            frontmatter: Frontmatter {
                title: "About".into(),
                ..Frontmatter::default()
            },
            raw_body: "body".into(),
            html_body: "<p>body</p>".into(),
            source_path: PathBuf::from("content/pages/about.md"),
            slug: "about".into(),
            collection: "pages".into(),
            url: "/about".into(),
            lang: "en".into(),
            excerpt: "body".into(),
            toc: vec![],
            word_count: 1,
            reading_time: 1,
            excerpt_html: "<p>body</p>".into(),
        };
        let items = vec![&item];
        let json = generate_search_index(&items, &config);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_generate_search_index_empty_items() {
        let config = minimal_config();
        let json = generate_search_index(&[], &config);
        assert_eq!(json, "[]");
    }

    // ── BuildStats::human_display ───────────────────────────────────────

    #[test]
    fn test_build_stats_human_display_basic() {
        let stats = BuildStats {
            items_built: {
                let mut m = HashMap::new();
                m.insert("posts".into(), 5);
                m
            },
            static_files_copied: 3,
            public_files_copied: 0,
            data_files_loaded: 0,
            duration_ms: 1500,
            step_timings: vec![],
            incremental: false,
            items_skipped: 0,
        };
        let display = stats.human_display();
        assert!(display.contains("5 posts"));
        assert!(display.contains("1.5s"));
        assert!(display.contains("3 static files copied"));
        assert!(!display.contains("public"));
        assert!(!display.contains("data"));
    }

    #[test]
    fn test_build_stats_human_display_with_public_and_data() {
        let stats = BuildStats {
            items_built: HashMap::new(),
            static_files_copied: 0,
            public_files_copied: 2,
            data_files_loaded: 4,
            duration_ms: 250,
            step_timings: vec![],
            incremental: false,
            items_skipped: 0,
        };
        let display = stats.human_display();
        assert!(display.contains("2 public files copied"));
        assert!(display.contains("4 data files loaded"));
    }

    #[test]
    fn test_build_stats_human_display_with_step_timings() {
        let stats = BuildStats {
            items_built: HashMap::new(),
            static_files_copied: 0,
            public_files_copied: 0,
            data_files_loaded: 0,
            duration_ms: 100,
            step_timings: vec![("Fast step".into(), 0.5), ("Slow step".into(), 15.3)],
            incremental: false,
            items_skipped: 0,
        };
        // Timings are verbose-only: not part of the summary line.
        assert!(!stats.human_display().contains("Timings:"));
        let timings = stats.timings_display().expect("timings recorded");
        assert!(timings.contains("Timings:"));
        assert!(timings.contains("Fast step: <1ms"));
        assert!(timings.contains("Slow step: 15.3ms"));
    }

    #[test]
    fn test_build_stats_timings_display_empty() {
        let stats = BuildStats {
            items_built: HashMap::new(),
            static_files_copied: 0,
            public_files_copied: 0,
            data_files_loaded: 0,
            duration_ms: 1,
            step_timings: vec![],
            incremental: false,
            items_skipped: 0,
        };
        assert!(stats.timings_display().is_none());
    }

    #[test]
    fn test_build_stats_human_display_incremental() {
        let stats = BuildStats {
            items_built: {
                let mut m = HashMap::new();
                m.insert("posts".into(), 20);
                m
            },
            static_files_copied: 3,
            public_files_copied: 0,
            data_files_loaded: 0,
            duration_ms: 300,
            step_timings: vec![],
            incremental: true,
            items_skipped: 18,
        };
        let display = stats.human_display();
        assert!(display.contains("Incremental build"));
        assert!(display.contains("18 items unchanged"));
        assert!(!display.contains("Built ")); // Should say "Incremental build" not "Built"
    }

    #[test]
    fn test_incremental_noop_refreshes_access_security_artifacts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = minimal_config();
        config.access = Some(AccessSection::default());
        config.collections[0].private = true;
        config.collections[0].access_group = Some("members".into());
        let paths = config.resolve_paths(tmp.path());
        fs::write(tmp.path().join("seite.toml"), "# unchanged\n").unwrap();

        let stale_private = paths.output.join("static/private/members");
        fs::create_dir_all(&stale_private).unwrap();
        fs::write(stale_private.join("confidential.webp"), "stale").unwrap();
        fs::write(
            paths.output.join("_worker.js"),
            "// Generated by seite password access. Do not edit.\n// old worker\n",
        )
        .unwrap();

        cache::BuildCache::snapshot(
            &tmp.path().join("seite.toml"),
            &paths.content,
            &paths.templates,
            &paths.data_dir,
            &paths.static_dir,
        )
        .save(tmp.path())
        .unwrap();

        let result = build_site(
            &config,
            &paths,
            &BuildOptions {
                include_drafts: false,
                incremental: true,
            },
        )
        .unwrap();

        assert!(result.stats.incremental);
        assert!(!paths.output.join("static/private").exists());
        let worker = fs::read_to_string(paths.output.join("_worker.js")).unwrap();
        assert!(worker.contains("function decodePath"));
        assert!(worker.contains("LEGACY_PRIVATE_PREFIX"));
        assert!(paths.output.join("_routes.json").exists());
    }

    #[test]
    fn test_incremental_noop_rejects_new_public_worker_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = minimal_config();
        config.access = Some(AccessSection::default());
        config.collections[0].private = true;
        config.collections[0].access_group = Some("members".into());
        let paths = config.resolve_paths(tmp.path());
        fs::write(tmp.path().join("seite.toml"), "# unchanged\n").unwrap();
        fs::create_dir_all(&paths.output).unwrap();
        cache::BuildCache::snapshot(
            &tmp.path().join("seite.toml"),
            &paths.content,
            &paths.templates,
            &paths.data_dir,
            &paths.static_dir,
        )
        .save(tmp.path())
        .unwrap();

        fs::create_dir_all(&paths.public_dir).unwrap();
        fs::write(
            paths.public_dir.join("_worker.js"),
            "export default { fetch() { return new Response('custom'); } };",
        )
        .unwrap();

        let result = build_site(
            &config,
            &paths,
            &BuildOptions {
                include_drafts: false,
                incremental: true,
            },
        );
        let error = match result {
            Ok(_) => panic!("custom public Worker should conflict with password access"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("public/_worker.js already exists"));
    }

    // ── SiteContext ─────────────────────────────────────────────────────

    #[test]
    fn test_site_context_from_config() {
        let config = minimal_config();
        let ctx = SiteContext::from_config(&config);
        assert_eq!(ctx.title, "Test Site");
        assert_eq!(ctx.description, "A test");
        assert_eq!(ctx.base_url, "https://example.com");
        assert_eq!(ctx.language, "en");
        assert_eq!(ctx.author, "Author");
    }

    #[test]
    fn test_site_context_for_lang_default() {
        let config = minimal_config();
        let ctx = SiteContext::for_lang(&config, "en");
        assert_eq!(ctx.title, "Test Site");
        assert_eq!(ctx.description, "A test");
    }

    #[test]
    fn test_site_context_for_lang_with_override() {
        let config = multilingual_config();
        let ctx = SiteContext::for_lang(&config, "es");
        assert_eq!(ctx.title, "Sitio de Prueba");
        assert_eq!(ctx.description, "Una prueba");
        assert_eq!(ctx.base_url, "https://example.com");
        // language should reflect the requested language, not the default
        assert_eq!(ctx.language, "es");
    }

    #[test]
    fn test_site_context_for_lang_no_override() {
        let config = multilingual_config();
        // "fr" is configured but has no title/description overrides
        let ctx = SiteContext::for_lang(&config, "fr");
        assert_eq!(ctx.title, "Test Site");
        assert_eq!(ctx.description, "A test");
    }

    // ── SiteContext::base_path ───────────────────────────────────────────

    #[test]
    fn test_site_context_base_path_root() {
        let config = minimal_config();
        let ctx = SiteContext::from_config(&config);
        assert_eq!(ctx.base_path, "");
    }

    #[test]
    fn test_site_context_base_path_subpath() {
        let mut config = minimal_config();
        config.site.base_url = "https://user.github.io/repo".into();
        let ctx = SiteContext::from_config(&config);
        assert_eq!(ctx.base_path, "/repo");
    }

    // ── needs_plausible_extensions_warning ─────────────────────────────

    #[test]
    fn test_plausible_warning_with_legacy_extensions() {
        let analytics = AnalyticsSection {
            provider: crate::config::AnalyticsProvider::Plausible,
            id: "example.com".into(),
            extensions: vec!["hash".into()],
            script_url: None,
            cookie_consent: false,
        };
        assert!(needs_plausible_extensions_warning(Some(&analytics)));
    }

    #[test]
    fn test_plausible_no_warning_with_script_url() {
        let analytics = AnalyticsSection {
            provider: crate::config::AnalyticsProvider::Plausible,
            id: "example.com".into(),
            extensions: vec!["hash".into()],
            script_url: Some("https://plausible.io/js/script.hash.js".into()),
            cookie_consent: false,
        };
        assert!(!needs_plausible_extensions_warning(Some(&analytics)));
    }

    #[test]
    fn test_plausible_no_warning_without_extensions() {
        let analytics = AnalyticsSection {
            provider: crate::config::AnalyticsProvider::Plausible,
            id: "example.com".into(),
            extensions: vec![],
            script_url: None,
            cookie_consent: false,
        };
        assert!(!needs_plausible_extensions_warning(Some(&analytics)));
    }

    #[test]
    fn test_plausible_no_warning_for_other_providers() {
        let analytics = AnalyticsSection {
            provider: crate::config::AnalyticsProvider::Google,
            id: "G-XXXXXXXX".into(),
            extensions: vec!["hash".into()],
            script_url: None,
            cookie_consent: false,
        };
        assert!(!needs_plausible_extensions_warning(Some(&analytics)));
    }

    #[test]
    fn test_plausible_no_warning_when_none() {
        assert!(!needs_plausible_extensions_warning(None));
    }

    // ── build_page_context ──────────────────────────────────────────────

    #[test]
    fn test_build_page_context_populates_required_fields() {
        let config = minimal_config();
        let site = SiteContext::from_config(&config);
        let data = serde_json::json!({});
        let item = ContentItem {
            frontmatter: Frontmatter {
                title: "My Post".into(),
                description: Some("A description".into()),
                date: Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
                updated: Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()),
                image: Some("/static/hero.png".into()),
                tags: vec!["rust".into(), "web".into()],
                robots: Some("noindex".into()),
                ..Frontmatter::default()
            },
            raw_body: "Some body text here for testing".into(),
            html_body: "<p>Some body text here for testing</p>".into(),
            source_path: PathBuf::from("content/posts/my-post.md"),
            slug: "my-post".into(),
            collection: "posts".into(),
            url: "/posts/my-post".into(),
            lang: "en".into(),
            excerpt: "Some body".into(),
            toc: vec![],
            word_count: 6,
            reading_time: 1,
            excerpt_html: "<p>Some body</p>".into(),
        };

        let ctx = build_page_context(&site, &item, &data, false);
        let json = ctx.into_json();
        let page = json.get("page").unwrap();
        assert_eq!(page["title"], "My Post");
        assert_eq!(page["description"], "A description");
        assert_eq!(page["date"], "2025-06-01");
        assert_eq!(page["updated"], "2025-06-15");
        assert_eq!(page["image"], "https://example.com/static/hero.png");
        assert_eq!(page["slug"], "my-post");
        assert_eq!(page["url"], "/posts/my-post");
        assert_eq!(page["collection"], "posts");
        assert_eq!(page["robots"], "noindex");
        assert_eq!(page["word_count"], 6);
        assert_eq!(page["reading_time"], 1);
        assert_eq!(page["tags"][0], "rust");
        assert_eq!(page["tags"][1], "web");
    }

    #[test]
    fn test_build_page_context_no_image() {
        let config = minimal_config();
        let site = SiteContext::from_config(&config);
        let data = serde_json::json!({});
        let item = ContentItem {
            frontmatter: Frontmatter {
                title: "No Image".into(),
                ..Frontmatter::default()
            },
            raw_body: String::new(),
            html_body: String::new(),
            source_path: PathBuf::from("content/pages/test.md"),
            slug: "test".into(),
            collection: "pages".into(),
            url: "/test".into(),
            lang: "en".into(),
            excerpt: String::new(),
            toc: vec![],
            word_count: 0,
            reading_time: 0,
            excerpt_html: String::new(),
        };

        let ctx = build_page_context(&site, &item, &data, false);
        let json = ctx.into_json();
        let page = json.get("page").unwrap();
        assert!(page["image"].is_null());
        assert!(page["date"].is_null());
        assert!(page["updated"].is_null());
    }

    // ── minify_css edge cases ───────────────────────────────────────────

    #[test]
    fn test_minify_css_preserves_content_without_spaces() {
        let input = b".cls{color:red;margin:0}";
        let out = minify_css(input);
        assert_eq!(out, ".cls{color:red;margin:0}");
    }

    #[test]
    fn test_minify_css_multiline() {
        let input = b"body {\n  color: red;\n  margin: 0;\n}\n\na {\n  color: blue;\n}";
        let out = minify_css(input);
        assert!(out.contains("body{color:red;margin:0;}"));
        assert!(out.contains("a{color:blue;}"));
    }

    // ── minify_js edge cases ────────────────────────────────────────────

    #[test]
    fn test_minify_js_multiple_comments() {
        let input = b"var a = 1; // first\nvar b = 2; // second\nvar c = 3;";
        let out = minify_js(input);
        assert!(!out.contains("first"));
        assert!(!out.contains("second"));
        assert!(out.contains("var a = 1;"));
        assert!(out.contains("var b = 2;"));
        assert!(out.contains("var c = 3;"));
    }

    #[test]
    fn test_minify_js_comment_at_start_of_line() {
        let input = b"// full line comment\nvar x = 1;";
        let out = minify_js(input);
        assert!(!out.contains("full line comment"));
        assert!(out.contains("var x = 1;"));
    }

    // ── resolve_slug edge cases ─────────────────────────────────────────

    #[test]
    fn test_resolve_slug_date_prefix_exact_boundary() {
        // Exactly 11 chars stem with proper separators at positions 4,7,10
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_posts();
        // "2025-01-15-" has 11 chars, the 12th char starts the slug
        let slug = resolve_slug(&fm, Path::new("2025-01-15-x.md"), &coll);
        assert_eq!(slug, "x");
    }

    #[test]
    fn test_resolve_slug_no_extension() {
        // Path with no extension — file_stem returns the whole filename
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let slug = resolve_slug(&fm, Path::new("readme"), &coll);
        assert_eq!(slug, "readme");
    }

    // ── TranslationLink serialization ───────────────────────────────────

    #[test]
    fn test_translation_link_serialization() {
        let link = TranslationLink {
            lang: "es".into(),
            url: "/es/about".into(),
        };
        let json = serde_json::to_value(&link).unwrap();
        assert_eq!(json["lang"], "es");
        assert_eq!(json["url"], "/es/about");
    }

    // ── Multiple items in generate_collection_index_md ───────────────────

    #[test]
    fn test_generate_collection_index_md_multiple_items() {
        let items = vec![
            ItemSummary {
                title: "First".into(),
                date: Some("2025-01-01".into()),
                description: None,
                slug: "first".into(),
                tags: vec![],
                url: "/posts/first".into(),
                word_count: 100,
                reading_time: 1,
                excerpt: String::new(),
            },
            ItemSummary {
                title: "Second".into(),
                date: None,
                description: Some("A second post".into()),
                slug: "second".into(),
                tags: vec![],
                url: "/posts/second".into(),
                word_count: 200,
                reading_time: 1,
                excerpt: String::new(),
            },
        ];
        let md = generate_collection_index_md("Posts", &items);
        assert!(md.contains("[First](/posts/first) (2025-01-01)"));
        assert!(md.contains("[Second](/posts/second)\n"));
        assert!(md.contains("  A second post\n"));
    }

    // ── PaginationContext serialization ──────────────────────────────────

    #[test]
    fn test_pagination_context_serialization_first_page() {
        let pg = PaginationContext {
            current_page: 1,
            total_pages: 3,
            prev_url: None,
            next_url: Some("/posts/page/2/".into()),
            base_url: "/posts".into(),
        };
        let json = serde_json::to_value(&pg).unwrap();
        assert_eq!(json["current_page"], 1);
        assert_eq!(json["total_pages"], 3);
        assert!(json["prev_url"].is_null());
        assert_eq!(json["next_url"], "/posts/page/2/");
        assert_eq!(json["base_url"], "/posts");
    }

    #[test]
    fn test_pagination_context_serialization_last_page() {
        let pg = PaginationContext {
            current_page: 3,
            total_pages: 3,
            prev_url: Some("/posts/page/2/".into()),
            next_url: None,
            base_url: "/posts".into(),
        };
        let json = serde_json::to_value(&pg).unwrap();
        assert_eq!(json["current_page"], 3);
        assert!(json["next_url"].is_null());
        assert_eq!(json["prev_url"], "/posts/page/2/");
    }

    #[test]
    fn test_pagination_context_serialization_middle_page() {
        let pg = PaginationContext {
            current_page: 2,
            total_pages: 5,
            prev_url: Some("/posts/".into()),
            next_url: Some("/posts/page/3/".into()),
            base_url: "/posts".into(),
        };
        let json = serde_json::to_value(&pg).unwrap();
        assert_eq!(json["current_page"], 2);
        assert_eq!(json["prev_url"], "/posts/");
        assert_eq!(json["next_url"], "/posts/page/3/");
    }

    // ── NavItem / NavSection serialization ──────────────────────────────

    #[test]
    fn test_nav_item_serialization() {
        let item = NavItem {
            title: "Getting Started".into(),
            url: "/docs/getting-started".into(),
            active: true,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["title"], "Getting Started");
        assert_eq!(json["url"], "/docs/getting-started");
        assert_eq!(json["active"], true);
    }

    #[test]
    fn test_nav_section_serialization() {
        let section = NavSection {
            name: "guides".into(),
            label: "Guides".into(),
            items: vec![
                NavItem {
                    title: "Setup".into(),
                    url: "/docs/guides/setup".into(),
                    active: false,
                },
                NavItem {
                    title: "Config".into(),
                    url: "/docs/guides/config".into(),
                    active: true,
                },
            ],
        };
        let json = serde_json::to_value(&section).unwrap();
        assert_eq!(json["name"], "guides");
        assert_eq!(json["label"], "Guides");
        assert_eq!(json["items"].as_array().unwrap().len(), 2);
        assert_eq!(json["items"][1]["active"], true);
    }

    // ── CollectionContext / ItemSummary serialization ────────────────────

    #[test]
    fn test_collection_context_serialization() {
        let ctx = CollectionContext {
            name: "posts".into(),
            label: "Blog Posts".into(),
            items: vec![ItemSummary {
                title: "Hello".into(),
                date: Some("2025-01-01".into()),
                description: None,
                slug: "hello".into(),
                tags: vec!["test".into()],
                url: "/posts/hello".into(),
                word_count: 50,
                reading_time: 1,
                excerpt: "<p>Hello</p>".into(),
            }],
        };
        let json = serde_json::to_value(&ctx).unwrap();
        assert_eq!(json["name"], "posts");
        assert_eq!(json["label"], "Blog Posts");
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        assert_eq!(json["items"][0]["word_count"], 50);
        assert_eq!(json["items"][0]["reading_time"], 1);
    }

    // ── PageContext serialization ────────────────────────────────────────

    #[test]
    fn test_page_context_serialization_complete() {
        let ctx = PageContext {
            title: "My Page".into(),
            content: "<p>Hello</p>".into(),
            date: Some("2025-01-01".into()),
            updated: Some("2025-06-01".into()),
            description: Some("A page".into()),
            image: Some("https://example.com/img.png".into()),
            slug: "my-page".into(),
            tags: vec!["tag1".into()],
            url: "/my-page".into(),
            collection: "pages".into(),
            robots: Some("noindex".into()),
            word_count: 100,
            reading_time: 1,
            excerpt: "<p>Hello</p>".into(),
            toc: vec![markdown::TocEntry {
                level: 2,
                text: "Section One".into(),
                id: "section-one".into(),
            }],
            extra: {
                let mut m = HashMap::new();
                m.insert("author".into(), serde_yaml_ng::Value::String("Jane".into()));
                m
            },
        };
        let json = serde_json::to_value(&ctx).unwrap();
        assert_eq!(json["title"], "My Page");
        assert_eq!(json["updated"], "2025-06-01");
        assert_eq!(json["robots"], "noindex");
        assert_eq!(json["toc"][0]["level"], 2);
        assert_eq!(json["toc"][0]["text"], "Section One");
        assert_eq!(json["toc"][0]["id"], "section-one");
        assert_eq!(json["extra"]["author"], "Jane");
    }

    #[test]
    fn test_page_context_serialization_minimal() {
        let ctx = PageContext {
            title: String::new(),
            content: String::new(),
            date: None,
            updated: None,
            description: None,
            image: None,
            slug: String::new(),
            tags: Vec::new(),
            url: "/".into(),
            collection: String::new(),
            robots: None,
            word_count: 0,
            reading_time: 0,
            excerpt: String::new(),
            toc: Vec::new(),
            extra: HashMap::new(),
        };
        let json = serde_json::to_value(&ctx).unwrap();
        assert!(json["date"].is_null());
        assert!(json["image"].is_null());
        assert!(json["robots"].is_null());
        assert_eq!(json["tags"].as_array().unwrap().len(), 0);
    }

    // ── SearchEntry serialization ───────────────────────────────────────

    #[test]
    fn test_search_entry_serialization() {
        let tags = vec!["rust".to_string(), "web".to_string()];
        let entry = SearchEntry {
            title: "Hello World",
            description: Some("A first post"),
            url: "/posts/hello",
            collection: "posts",
            tags: &tags,
            date: Some("2025-01-01".into()),
            lang: "en",
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["title"], "Hello World");
        assert_eq!(json["description"], "A first post");
        assert_eq!(json["tags"].as_array().unwrap().len(), 2);
        assert_eq!(json["lang"], "en");
    }

    #[test]
    fn test_search_entry_serialization_no_description() {
        let tags: Vec<String> = vec![];
        let entry = SearchEntry {
            title: "About",
            description: None,
            url: "/about",
            collection: "pages",
            tags: &tags,
            date: None,
            lang: "en",
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json["description"].is_null());
        assert!(json["date"].is_null());
    }

    // ── generate_search_index with multiple collections ─────────────────

    #[test]
    fn test_generate_search_index_mixed_listed_unlisted() {
        let mut config = minimal_config();
        config.collections = vec![
            CollectionConfig::preset_posts(), // listed=true
            CollectionConfig::preset_pages(), // listed=false
        ];

        let post = ContentItem {
            frontmatter: Frontmatter {
                title: "Post".into(),
                ..Frontmatter::default()
            },
            raw_body: "body".into(),
            html_body: "<p>body</p>".into(),
            source_path: PathBuf::from("content/posts/post.md"),
            slug: "post".into(),
            collection: "posts".into(),
            url: "/posts/post".into(),
            lang: "en".into(),
            excerpt: String::new(),
            toc: vec![],
            word_count: 1,
            reading_time: 1,
            excerpt_html: String::new(),
        };

        let page = ContentItem {
            frontmatter: Frontmatter {
                title: "Page".into(),
                ..Frontmatter::default()
            },
            raw_body: "body".into(),
            html_body: "<p>body</p>".into(),
            source_path: PathBuf::from("content/pages/page.md"),
            slug: "page".into(),
            collection: "pages".into(),
            url: "/page".into(),
            lang: "en".into(),
            excerpt: String::new(),
            toc: vec![],
            word_count: 1,
            reading_time: 1,
            excerpt_html: String::new(),
        };

        let items = vec![&post, &page];
        let json = generate_search_index(&items, &config);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        // Only the post should be included (listed=true)
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["title"], "Post");
    }

    // ── fnv_hash8 known value ───────────────────────────────────────────

    #[test]
    fn test_fnv_hash8_known_value() {
        // Verify hash is hex and consistent
        let h = fnv_hash8(b"test");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Run twice to verify consistency
        assert_eq!(h, fnv_hash8(b"test"));
    }

    // ── build_url with nested slug ──────────────────────────────────────

    #[test]
    fn test_build_url_nested_slug() {
        assert_eq!(build_url("/docs", "guides/setup"), "/docs/guides/setup");
    }

    #[test]
    fn test_build_url_with_only_slash_prefix() {
        assert_eq!(build_url("/", "about"), "/about");
    }

    // ── title_case with unicode ─────────────────────────────────────────

    #[test]
    fn test_title_case_single_char_words() {
        assert_eq!(title_case("a-b"), "A B");
    }

    // ── minify_css with only whitespace ─────────────────────────────────

    #[test]
    fn test_minify_css_only_whitespace() {
        let out = minify_css(b"   \n\n  \t ");
        assert_eq!(out, "");
    }

    #[test]
    fn test_minify_css_only_comment() {
        let out = minify_css(b"/* entire file is a comment */");
        assert_eq!(out, "");
    }

    // ── url_to_output_path with lang prefix ─────────────────────────────

    #[test]
    fn test_url_to_output_path_with_lang_prefix() {
        let p = url_to_output_path(Path::new("/out"), "/es/posts/hello");
        assert_eq!(p, PathBuf::from("/out/es/posts/hello.html"));
    }

    #[test]
    fn test_url_to_md_path_with_lang_prefix() {
        let p = url_to_md_path(Path::new("/out"), "/fr/docs/setup");
        assert_eq!(p, PathBuf::from("/out/fr/docs/setup.md"));
    }

    // ── resolve_slug_i18n with non-matching lang suffix ─────────────────

    #[test]
    fn test_resolve_slug_i18n_non_matching_suffix_preserved() {
        let fm = default_frontmatter();
        let coll = CollectionConfig::preset_pages();
        let langs: HashSet<&str> = ["es"].into_iter().collect();
        // "about.min" — "min" is not a configured language
        let slug = resolve_slug_i18n(&fm, Path::new("about.min.md"), &coll, &langs);
        assert_eq!(slug, "about.min");
    }

    // ── ui_strings completeness ─────────────────────────────────────────

    #[test]
    fn test_ui_strings_for_lang_contains_all_expected_keys() {
        let data = serde_json::json!({});
        let t = ui_strings_for_lang("en", &data);
        let obj = t.as_object().unwrap();
        let expected_keys = [
            "search_placeholder",
            "skip_to_content",
            "no_results",
            "newer",
            "older",
            "page_n_of_total",
            "search_label",
            "min_read",
            "contents",
            "tags",
            "all_tags",
            "tagged",
            "changelog",
            "all_releases",
            "roadmap",
            "not_found_title",
            "not_found_message",
            "go_home",
            "in_progress",
            "planned",
            "done",
            "other",
            "trust_center",
            "trust_hero_subtitle",
            "certifications_compliance",
            "active",
            "learn_more",
            "auditor",
            "scope",
            "issued",
            "expires",
            "subprocessors",
            "vendor",
            "purpose",
            "location",
            "dpa",
            "yes",
            "no",
            "faq",
            "resources",
            "previous",
            "next",
            "prev_post",
            "next_post",
            "on_this_page",
            "search_docs",
            "search_documentation",
            "toggle_theme",
            "toggle_sidebar",
            "built_with",
            "get_started",
            "view_on_github",
            "rss",
            "changelog_subtitle",
            "roadmap_subtitle",
            "open_an_issue",
            "have_a_feature_request",
            "contact_name",
            "contact_email",
            "contact_message",
            "contact_submit",
            "documentation",
            "callout_info",
            "callout_tip",
            "callout_warning",
            "callout_danger",
        ];
        for key in expected_keys {
            assert!(obj.contains_key(key), "Missing UI string key: {key}");
        }
    }

    // ── AdjacentPost serialization ─────────────────────────────────────

    #[test]
    fn test_adjacent_post_serialization() {
        let post = AdjacentPost {
            title: "Hello World".into(),
            url: "/posts/hello-world/".into(),
            date: Some("2025-06-01".into()),
            description: Some("A post".into()),
        };
        let json = serde_json::to_value(&post).unwrap();
        assert_eq!(json["title"], "Hello World");
        assert_eq!(json["url"], "/posts/hello-world/");
        assert_eq!(json["date"], "2025-06-01");
        assert_eq!(json["description"], "A post");
    }

    #[test]
    fn test_adjacent_post_none_fields() {
        let post = AdjacentPost {
            title: "No Date".into(),
            url: "/posts/no-date/".into(),
            date: None,
            description: None,
        };
        let json = serde_json::to_value(&post).unwrap();
        assert_eq!(json["title"], "No Date");
        assert!(json["date"].is_null());
        assert!(json["description"].is_null());
    }

    fn staging_siblings(parent: &Path) -> Vec<String> {
        fs::read_dir(parent)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".staging-") || n.contains(".old-"))
            .collect()
    }

    fn write_post(paths: &ResolvedPaths, name: &str, body: &str) {
        let dir = paths.content.join("posts");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), body).unwrap();
    }

    fn full_build(config: &SiteConfig, paths: &ResolvedPaths) -> Result<BuildResult> {
        build_site(
            config,
            paths,
            &BuildOptions {
                include_drafts: false,
                incremental: false,
            },
        )
    }

    #[test]
    fn test_is_scratch_output_name() {
        assert!(is_scratch_output_name("dist.staging-123", "dist"));
        assert!(is_scratch_output_name("dist.old-9", "dist"));
        assert!(!is_scratch_output_name("dist.staging-", "dist"));
        assert!(!is_scratch_output_name("dist.staging-12a", "dist"));
        assert!(!is_scratch_output_name("dist", "dist"));
        assert!(!is_scratch_output_name("public.staging-1", "dist"));
    }

    #[test]
    fn test_is_build_output_path() {
        let config = minimal_config();
        let paths = config.resolve_paths(Path::new("/site"));
        assert!(is_build_output_path(
            &paths,
            Path::new("/site/dist/index.html")
        ));
        assert!(is_build_output_path(
            &paths,
            Path::new("/site/dist.staging-42/posts/a.html")
        ));
        assert!(is_build_output_path(
            &paths,
            Path::new("/site/dist-subdomains/docs.staging-1/x.html")
        ));
        assert!(!is_build_output_path(
            &paths,
            Path::new("/site/content/posts/a.md")
        ));
        assert!(!is_build_output_path(
            &paths,
            Path::new("/site/distribution/a")
        ));
    }

    #[test]
    fn test_full_build_replaces_output_via_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nHi\n",
        );
        fs::create_dir_all(&paths.output).unwrap();
        fs::write(paths.output.join("stale.html"), "old").unwrap();

        full_build(&config, &paths).unwrap();

        assert!(paths.output.join("index.html").exists());
        assert!(paths.output.join("posts/hello.html").exists());
        assert!(
            !paths.output.join("stale.html").exists(),
            "full builds start from a clean output"
        );
        assert!(staging_siblings(tmp.path()).is_empty());
    }

    #[test]
    fn test_failed_full_build_keeps_previous_output() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nHi\n",
        );
        full_build(&config, &paths).unwrap();
        let index_before = fs::read_to_string(paths.output.join("index.html")).unwrap();

        write_post(&paths, "bad.md", "---\ntitle: [unclosed\n---\nbody\n");
        assert!(full_build(&config, &paths).is_err());

        assert_eq!(
            fs::read_to_string(paths.output.join("index.html")).unwrap(),
            index_before,
            "a failed build must leave the previous output untouched"
        );
        assert!(paths.output.join("posts/hello.html").exists());
        assert!(staging_siblings(tmp.path()).is_empty());
    }

    #[test]
    fn test_build_removes_leftover_staging_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nHi\n",
        );
        fs::create_dir_all(tmp.path().join("dist.staging-999999/posts")).unwrap();
        fs::create_dir_all(tmp.path().join("dist.old-999999")).unwrap();
        fs::create_dir_all(tmp.path().join("dist.staging-notes")).unwrap();

        full_build(&config, &paths).unwrap();

        assert!(staging_siblings(tmp.path()) == vec!["dist.staging-notes".to_string()]);
    }

    #[cfg(unix)]
    #[test]
    fn test_build_keeps_staging_dir_of_running_build() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nHi\n",
        );
        // A live process other than this one (pid 1 always exists on unix).
        fs::create_dir_all(tmp.path().join("dist.staging-1/posts")).unwrap();

        full_build(&config, &paths).unwrap();

        assert!(staging_siblings(tmp.path()) == vec!["dist.staging-1".to_string()]);
    }

    #[test]
    fn test_incremental_content_build_updates_output_in_place() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        fs::write(tmp.path().join("seite.toml"), "# config\n").unwrap();
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nHi\n",
        );
        let incremental = BuildOptions {
            include_drafts: false,
            incremental: true,
        };
        build_site(&config, &paths, &incremental).unwrap();
        // A file only the previous output has: an in-place build keeps it.
        fs::write(paths.output.join("marker.txt"), "kept").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_post(
            &paths,
            "hello.md",
            "---\ntitle: Hello\ndate: 2025-01-01\n---\nChanged body\n",
        );
        let result = build_site(&config, &paths, &incremental).unwrap();

        assert!(result.stats.incremental);
        assert!(paths.output.join("marker.txt").exists());
        assert!(fs::read_to_string(paths.output.join("posts/hello.html"))
            .unwrap()
            .contains("Changed body"));
        assert!(staging_siblings(tmp.path()).is_empty());
    }

    #[test]
    fn test_swap_into_place_without_existing_output() {
        let tmp = tempfile::tempdir().unwrap();
        let staging = tmp.path().join("out.staging-1");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("a.html"), "new").unwrap();
        let output = tmp.path().join("out");

        swap_into_place(tmp.path(), &staging, &output).unwrap();

        assert_eq!(fs::read_to_string(output.join("a.html")).unwrap(), "new");
        assert!(!staging.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_swap_into_place_keeps_symlinked_output() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real-out");
        fs::create_dir_all(real.join("old-dir")).unwrap();
        fs::write(real.join("old.html"), "old").unwrap();
        let output = tmp.path().join("out");
        std::os::unix::fs::symlink(&real, &output).unwrap();
        let staging = tmp.path().join("out.staging-1");
        fs::create_dir_all(staging.join("posts")).unwrap();
        fs::write(staging.join("posts/a.html"), "new").unwrap();

        swap_into_place(tmp.path(), &staging, &output).unwrap();

        assert!(fs::symlink_metadata(&output)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(real.join("posts/a.html")).unwrap(),
            "new"
        );
        assert!(!real.join("old.html").exists());
        assert!(!real.join("old-dir").exists());
        assert!(!staging.exists());
    }

    #[test]
    fn test_build_attributes_broken_links_to_markdown_source() {
        let tmp = tempfile::tempdir().unwrap();
        let config = minimal_config();
        let paths = config.resolve_paths(tmp.path());
        write_post(
            &paths,
            "2025-01-01-a.md",
            "---\ntitle: A\ndate: 2025-01-01\n---\n\nIntro\n\n[gone](/posts/gone)\n\n![x](/static/nope.png)\n\n[b](./2025-01-02-b.md#top) [bad](./missing.md)\n",
        );
        write_post(
            &paths,
            "2025-01-02-b.md",
            "---\ntitle: B\ndate: 2025-01-02\n---\nB\n",
        );

        let result = full_build(&config, &paths).unwrap();
        let check = &result.link_check;

        let gone = check
            .broken_links
            .iter()
            .find(|l| l.href == "/posts/gone")
            .expect("broken link reported");
        assert_eq!(gone.location(), "content/posts/2025-01-01-a.md:8");

        let md = check
            .broken_links
            .iter()
            .find(|l| l.href == "./missing.md")
            .expect("unresolved .md link reported");
        assert_eq!(md.kind, links::LinkKind::Markdown);
        assert_eq!(md.line, Some(12));

        assert_eq!(check.missing_assets.len(), 1);
        assert_eq!(
            check.missing_assets[0].location(),
            "content/posts/2025-01-01-a.md:10"
        );

        let html = fs::read_to_string(paths.output.join("posts/a.html")).unwrap();
        assert!(html.contains(r#"href="/posts/b#top""#), "{html}");
    }
}
