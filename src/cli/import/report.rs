//! Human summary and the Markdown report (`.seite/cache/import-report.md`)
//! of `seite import`. No timestamps or absolute paths: the report is part of
//! the deterministic output.

use std::fmt::Write as _;

use super::{Report, SourceKind};
use crate::output::human;

/// Warnings shown in the terminal before "… and N more (see the report)".
const SHOWN_WARNINGS: usize = 20;

pub(crate) fn print_human(report: &Report, kind: SourceKind) {
    let verb = if report.dry_run {
        "Would import"
    } else {
        "Imported"
    };
    human::header(&format!(
        "{verb} from {} ({})",
        kind.label(),
        report.source_path
    ));
    for (kind, c) in &report.summary {
        if c.imported + c.skipped + c.failed == 0 {
            continue;
        }
        crate::human_println!(
            "  {kind}: {} {}, {} skipped, {} failed",
            c.imported,
            if report.dry_run {
                "to import"
            } else {
                "imported"
            },
            c.skipped,
            c.failed
        );
    }
    let i = &report.images;
    if report.dry_run {
        crate::human_println!(
            "  images: {} to download or copy, {} not imported",
            i.planned,
            i.refused
        );
    } else {
        crate::human_println!(
            "  images: {} downloaded, {} copied, {} deduplicated, {} failed, {} not imported",
            i.downloaded,
            i.copied,
            i.deduplicated,
            i.failed,
            i.refused
        );
    }
    for item in report.items.iter().filter(|i| i.status == "failed") {
        human::error(&format!(
            "{}: {}",
            item.source,
            item.reason.as_deref().unwrap_or("failed")
        ));
    }
    if !report.warnings.is_empty() {
        crate::human_println!();
        crate::human_println!("  {} warning(s) to review:", report.warnings.len());
        for w in report.warnings.iter().take(SHOWN_WARNINGS) {
            crate::human_println!("  {}", one_line(w));
        }
        if report.warnings.len() > SHOWN_WARNINGS {
            crate::human_println!(
                "  … and {} more{}",
                report.warnings.len() - SHOWN_WARNINGS,
                if report.report.is_some() {
                    " (see the report)"
                } else {
                    ""
                }
            );
        }
    }
    crate::human_println!();
    if let Some(path) = &report.report {
        human::info(&format!("Report: {path} (every item and warning)"));
    }
    if report.dry_run {
        human::info("Dry run: nothing was written. Run again without --dry-run to import.");
    } else {
        human::info("Next: `seite check`, then `/seite check` in your coding agent to work through the warnings");
    }
}

fn one_line(w: &super::Warning) -> String {
    let path = w
        .path
        .as_deref()
        .map(|p| format!(" → {p}"))
        .unwrap_or_default();
    format!("{}: [{}] {}{path}", w.source, w.code, w.message)
}

/// The report file: summary, a warning checklist, then every item.
pub(crate) fn markdown(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Import report\n");
    let _ = writeln!(
        out,
        "Imported from {} (`{}`) with `seite import`. Work through the checklist, \
         then run `seite check` (and `seite build`) until both are clean.\n",
        report.source.label(),
        report.source_path
    );
    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(out, "| | imported | skipped | failed |\n|---|---|---|---|");
    for (kind, c) in &report.summary {
        let _ = writeln!(
            out,
            "| {kind} | {} | {} | {} |",
            c.imported, c.skipped, c.failed
        );
    }
    let i = &report.images;
    let _ = writeln!(
        out,
        "\nImages: {} downloaded, {} copied, {} deduplicated, {} failed, {} not imported.\n",
        i.downloaded, i.copied, i.deduplicated, i.failed, i.refused
    );

    let _ = writeln!(out, "## Warnings\n");
    if report.warnings.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        for w in &report.warnings {
            let path = w
                .path
                .as_deref()
                .map(|p| format!(" — fix in `{p}`"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "- [ ] `{}` **{}**: {}{path}",
                w.source,
                w.code,
                w.message.replace('\n', " ")
            );
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "## Items\n");
    let _ = writeln!(
        out,
        "| status | source | title | written to | url | aliases |\n|---|---|---|---|---|---|"
    );
    for item in &report.items {
        let status = match &item.reason {
            Some(r) => format!("{} ({r})", item.status),
            None => item.status.to_string(),
        };
        let _ = writeln!(
            out,
            "| {} | `{}` | {} | {} | {} | {} |",
            cell(&status),
            item.source,
            cell(&item.title),
            item.path
                .as_deref()
                .map(|p| format!("`{p}`"))
                .unwrap_or_default(),
            item.url.as_deref().unwrap_or(""),
            cell(&item.aliases.join(", "))
        );
    }
    out
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}
