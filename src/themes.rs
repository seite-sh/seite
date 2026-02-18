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

pub fn all() -> Vec<Theme> {
    vec![default(), minimal(), dark(), docs(), brutalist(), bento()]
}

pub fn by_name(name: &str) -> Option<Theme> {
    all().into_iter().find(|t| t.name == name)
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
