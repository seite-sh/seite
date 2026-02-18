/// Bundled themes. Each is a self-contained base.html Tera template embedded
/// at compile time via include_str!. Binary ships with all themes — no downloads needed.
///
/// To edit a theme, modify the corresponding file in src/themes/:
///   default.tera, minimal.tera, dark.tera, docs.tera, brutalist.tera, bento.tera
pub struct Theme {
    pub name: &'static str,
    pub description: &'static str,
    pub base_html: &'static str,
}

/// An installed theme loaded from `templates/themes/<name>.tera` on disk.
pub struct InstalledTheme {
    pub name: String,
    pub description: String,
    pub base_html: String,
}

pub fn all() -> Vec<Theme> {
    vec![default(), minimal(), dark(), docs(), brutalist(), bento()]
}

pub fn by_name(name: &str) -> Option<Theme> {
    all().into_iter().find(|t| t.name == name)
}

/// Discover installed themes from `templates/themes/*.tera` in the given project root.
pub fn installed_themes(project_root: &std::path::Path) -> Vec<InstalledTheme> {
    let themes_dir = project_root.join("templates").join("themes");
    let mut themes = Vec::new();

    let entries = match std::fs::read_dir(&themes_dir) {
        Ok(entries) => entries,
        Err(_) => return themes,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tera") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let description = parse_theme_description(&content)
            .unwrap_or_else(|| "Installed theme".to_string());
        themes.push(InstalledTheme {
            name,
            description,
            base_html: content,
        });
    }

    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}

/// Find an installed theme by name in the given project root.
pub fn installed_by_name(project_root: &std::path::Path, name: &str) -> Option<InstalledTheme> {
    let path = project_root
        .join("templates")
        .join("themes")
        .join(format!("{name}.tera"));
    let content = std::fs::read_to_string(&path).ok()?;
    let description = parse_theme_description(&content)
        .unwrap_or_else(|| "Installed theme".to_string());
    Some(InstalledTheme {
        name: name.to_string(),
        description,
        base_html: content,
    })
}

/// Parse a description from theme metadata comments.
/// Looks for `{#- theme-description: ... -#}` at the top of the file.
fn parse_theme_description(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("{#-") {
            if let Some(rest) = rest.strip_suffix("-#}") {
                let rest = rest.trim();
                if let Some(desc) = rest.strip_prefix("theme-description:") {
                    return Some(desc.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn default() -> Theme {
    Theme {
        name: "default",
        description: "Clean, readable theme with system fonts",
        base_html: include_str!("themes/default.tera"),
    }
}

pub fn minimal() -> Theme {
    Theme {
        name: "minimal",
        description: "Ultra-minimal, typography-first theme",
        base_html: include_str!("themes/minimal.tera"),
    }
}

pub fn dark() -> Theme {
    Theme {
        name: "dark",
        description: "Dark mode theme, easy on the eyes",
        base_html: include_str!("themes/dark.tera"),
    }
}

pub fn docs() -> Theme {
    Theme {
        name: "docs",
        description: "Documentation-focused theme with sidebar layout",
        base_html: include_str!("themes/docs.tera"),
    }
}

pub fn brutalist() -> Theme {
    Theme {
        name: "brutalist",
        description: "Neo-brutalist theme with thick borders and hard shadows",
        base_html: include_str!("themes/brutalist.tera"),
    }
}

pub fn bento() -> Theme {
    Theme {
        name: "bento",
        description: "Card grid layout inspired by bento box design",
        base_html: include_str!("themes/bento.tera"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_theme_description() {
        let content = "{#- theme-description: A cool dark theme -#}\n<!DOCTYPE html>";
        assert_eq!(
            parse_theme_description(content),
            Some("A cool dark theme".to_string())
        );
    }

    #[test]
    fn test_parse_theme_description_missing() {
        let content = "<!DOCTYPE html>\n<html>";
        assert_eq!(parse_theme_description(content), None);
    }

    #[test]
    fn test_all_themes_have_unique_names() {
        let themes = all();
        let mut names: Vec<_> = themes.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 6);
    }

    #[test]
    fn test_by_name_found() {
        assert!(by_name("dark").is_some());
        assert!(by_name("brutalist").is_some());
    }

    #[test]
    fn test_by_name_not_found() {
        assert!(by_name("nonexistent").is_none());
    }
}
