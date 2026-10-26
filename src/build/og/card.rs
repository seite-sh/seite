//! Card layout: text measurement and wrapping, the three built-in layouts as
//! SVG, and rasterization to PNG with resvg.
//!
//! Everything here is deterministic: text is measured with the bundled (or
//! configured) font's own advance widths, the font database holds only those
//! faces (never system fonts), and the SVG is built from fixed coordinates.

use std::sync::Arc;

use resvg::tiny_skia;
use resvg::usvg;
use resvg::usvg::fontdb;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

use crate::config::OgTemplate;

pub const WIDTH: u32 = 1200;
pub const HEIGHT: u32 = 630;
const PAD: f32 = 80.0;

/// The bundled faces: Inter Display Bold (titles) and Inter Medium (everything
/// else), Latin/Latin Extended/Greek/Cyrillic subsets of Inter 4.1 (SIL OFL
/// 1.1, `assets/fonts/OFL.txt`). Subset with fonttools `pyftsubset
/// --no-hinting --desubroutinize --layout-features=kern,liga,calt,ccmp,locl,mark,mkmk`.
pub const BUNDLED_TITLE: &[u8] = include_bytes!("../../../assets/fonts/InterDisplay-Bold.ttf");
pub const BUNDLED_BODY: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");

/// An sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parse `#rgb` or `#rrggbb` (case-insensitive).
    pub fn parse(s: &str) -> Option<Rgb> {
        let hex = s.trim().strip_prefix('#')?;
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let v = |s: &str| u8::from_str_radix(s, 16).ok();
        match hex.len() {
            3 => {
                let c: Vec<u8> = hex
                    .chars()
                    .map(|c| v(&c.to_string()).map(|n| n * 17))
                    .collect::<Option<_>>()?;
                Some(Rgb(c[0], c[1], c[2]))
            }
            6 => Some(Rgb(v(&hex[0..2])?, v(&hex[2..4])?, v(&hex[4..6])?)),
            _ => None,
        }
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// WCAG relative luminance.
    fn luminance(self) -> f64 {
        let ch = |c: u8| {
            let c = c as f64 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(self.0) + 0.7152 * ch(self.1) + 0.0722 * ch(self.2)
    }

    /// WCAG contrast ratio.
    pub fn contrast(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// Card colors and layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub template: OgTemplate,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
}

/// A face used for measuring and rendering.
pub struct Face {
    pub data: Arc<Vec<u8>>,
    pub family: String,
}

/// A parsed face, ready for measuring.
pub(crate) struct Measure<'a> {
    font: FontRef<'a>,
}

impl Face {
    pub(crate) fn measure(&self) -> Option<Measure<'_>> {
        FontRef::new(&self.data).ok().map(|font| Measure { font })
    }
}

impl Measure<'_> {
    fn has(&self, c: char) -> bool {
        self.font.charmap().map(c).is_some()
    }

    /// Width of `text` at `size` px (advance widths, kerning ignored -- it
    /// only narrows text, so the measurement errs on the safe side).
    /// Characters the face lacks count as an average-width glyph.
    fn width(&self, text: &str, size: f32) -> f32 {
        let charmap = self.font.charmap();
        let metrics = self
            .font
            .glyph_metrics(Size::new(size), LocationRef::default());
        let fallback = charmap
            .map('n')
            .and_then(|g| metrics.advance_width(g))
            .unwrap_or(size * 0.55);
        text.chars()
            .map(|c| {
                charmap
                    .map(c)
                    .and_then(|g| metrics.advance_width(g))
                    .unwrap_or(fallback)
            })
            .sum()
    }
}

/// The fonts a card is drawn with: the configured font (if any) instead of
/// the bundled faces, which stay loaded as glyph fallback. The `fontdb`
/// holds only these faces -- never system fonts -- so cards render the same
/// on every machine.
pub struct Fonts {
    pub db: Arc<fontdb::Database>,
    /// Title face (user font or Inter Display Bold).
    pub title: Face,
    /// Body face (user font or Inter Medium).
    pub body: Face,
    /// Every face, for glyph coverage (user font first).
    coverage: Vec<Arc<Vec<u8>>>,
}

/// Load `data` into `db` and return the family name fontdb gives it.
fn load_face(db: &mut fontdb::Database, data: &[u8]) -> Option<String> {
    let before = db.len();
    db.load_font_data(data.to_vec());
    if db.len() == before {
        return None;
    }
    db.faces()
        .last()?
        .families
        .first()
        .map(|(name, _)| name.clone())
}

impl Fonts {
    /// The bundled faces plus an optional user font. `Err` when the user font
    /// cannot be parsed.
    pub fn load(user: Option<Vec<u8>>) -> Result<Fonts, String> {
        let mut db = fontdb::Database::new();
        let title_data = Arc::new(BUNDLED_TITLE.to_vec());
        let body_data = Arc::new(BUNDLED_BODY.to_vec());
        let mut title = Face {
            family: load_face(&mut db, BUNDLED_TITLE).unwrap_or_else(|| "Inter Display".into()),
            data: title_data.clone(),
        };
        let mut body = Face {
            family: load_face(&mut db, BUNDLED_BODY).unwrap_or_else(|| "Inter".into()),
            data: body_data.clone(),
        };
        let mut coverage = vec![title_data, body_data];
        if let Some(bytes) = user {
            if FontRef::new(&bytes).is_err() {
                return Err("not a TrueType/OpenType font".into());
            }
            let family = load_face(&mut db, &bytes).ok_or_else(|| {
                "not a TrueType/OpenType font (or it has no family name)".to_string()
            })?;
            let data = Arc::new(bytes);
            title = Face {
                data: data.clone(),
                family: family.clone(),
            };
            body = Face {
                data: data.clone(),
                family,
            };
            coverage.insert(0, data);
        }
        Ok(Fonts {
            db: Arc::new(db),
            title,
            body,
            coverage,
        })
    }

    /// Whether every visible character of `text` has a glyph in one of the
    /// faces.
    pub fn covers(&self, text: &str) -> bool {
        let faces: Vec<Measure<'_>> = self
            .coverage
            .iter()
            .filter_map(|d| FontRef::new(d).ok().map(|font| Measure { font }))
            .collect();
        text.chars().all(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '\u{200b}'..='\u{200f}' | '\u{fe00}'..='\u{fe0f}')
                || faces.iter().any(|f| f.has(c))
        })
    }
}

/// Wrap `text` into lines no wider than `max_width` at `size`. Words longer
/// than a line are broken between characters.
fn wrap(face: &Measure<'_>, text: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if face.width(&candidate, size) <= max_width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if face.width(word, size) <= max_width {
            current = word.to_string();
        } else {
            // Break an overlong word between characters.
            for c in word.chars() {
                let mut next = current.clone();
                next.push(c);
                if !current.is_empty() && face.width(&next, size) > max_width {
                    lines.push(std::mem::take(&mut current));
                    current.push(c);
                } else {
                    current = next;
                }
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Fit `text` into at most `max_lines` lines, trying each size in `sizes`
/// (largest first); at the smallest size, the last line ends with an
/// ellipsis. Returns the chosen size and lines.
pub(crate) fn fit(
    face: &Measure<'_>,
    text: &str,
    sizes: &[f32],
    max_width: f32,
    max_lines: usize,
) -> (f32, Vec<String>) {
    for &size in sizes {
        let lines = wrap(face, text, size, max_width);
        if lines.len() <= max_lines {
            return (size, lines);
        }
    }
    let size = *sizes.last().unwrap_or(&48.0);
    let mut lines = wrap(face, text, size, max_width);
    lines.truncate(max_lines);
    if let Some(last) = lines.last_mut() {
        *last = ellipsize(face, last, size, max_width);
    }
    (size, lines)
}

/// `line` + "…", shortened (by words, then characters) until it fits.
fn ellipsize(face: &Measure<'_>, line: &str, size: f32, max_width: f32) -> String {
    let mut base = line.trim_end().to_string();
    loop {
        let candidate = format!("{}…", base.trim_end_matches([' ', ',', '.', ':', ';', '-']));
        if face.width(&candidate, size) <= max_width || base.is_empty() {
            return candidate;
        }
        match base.rfind(' ') {
            Some(i) if i > 0 && base.len() - i < 12 => base.truncate(i),
            _ => {
                base.pop();
            }
        }
    }
}

/// What a card says.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardText {
    /// Small header line (site name on page cards, host on the home card).
    pub header: String,
    /// The big text.
    pub title: String,
    /// Optional second paragraph under the title (home card description).
    pub subtitle: Option<String>,
    /// Footer metadata ("Posts · 2026-10-22 · 5 min read"); may be empty.
    pub meta: String,
}

/// A logo to draw on the card.
pub enum Logo {
    Svg(Box<usvg::Tree>),
    Png(tiny_skia::Pixmap),
}

impl Logo {
    /// Parse logo bytes (`is_svg` from the file extension).
    pub fn parse(bytes: &[u8], is_svg: bool, fonts: &Fonts) -> Result<Logo, String> {
        if is_svg {
            let opt = usvg::Options {
                fontdb: fonts.db.clone(),
                font_family: fonts.body.family.clone(),
                ..Default::default()
            };
            usvg::Tree::from_data(bytes, &opt)
                .map(|tree| Logo::Svg(Box::new(tree)))
                .map_err(|e| format!("not a valid SVG: {e}"))
        } else {
            tiny_skia::Pixmap::decode_png(bytes)
                .map(Logo::Png)
                .map_err(|e| format!("not a valid PNG: {e}"))
        }
    }

    fn size(&self) -> (f32, f32) {
        match self {
            Logo::Svg(tree) => (tree.size().width(), tree.size().height()),
            Logo::Png(p) => (p.width() as f32, p.height() as f32),
        }
    }

    /// Width the logo takes when drawn `height` px tall (capped at 4:1).
    fn width_at(&self, height: f32) -> f32 {
        let (w, h) = self.size();
        if w <= 0.0 || h <= 0.0 {
            return 0.0;
        }
        (w / h * height).min(height * 4.0)
    }

    fn draw(&self, pixmap: &mut tiny_skia::Pixmap, x: f32, y: f32, height: f32) {
        let (w, h) = self.size();
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let scale = (height / h).min(height * 4.0 / w);
        let dy = (height - h * scale) / 2.0;
        let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y + dy);
        match self {
            Logo::Svg(tree) => resvg::render(tree, transform, &mut pixmap.as_mut()),
            Logo::Png(png) => pixmap.draw_pixmap(
                0,
                0,
                png.as_ref(),
                &tiny_skia::PixmapPaint {
                    quality: tiny_skia::FilterQuality::Bicubic,
                    ..Default::default()
                },
                transform,
                None,
            ),
        }
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// One `<text>` element.
#[allow(clippy::too_many_arguments)]
fn text_el(
    svg: &mut String,
    x: f32,
    y: f32,
    family: &str,
    weight: u16,
    size: f32,
    fill: Rgb,
    opacity: f32,
    anchor: &str,
    text: &str,
) {
    svg.push_str(&format!(
        "<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"'{}'\" font-weight=\"{weight}\" font-size=\"{size:.1}\" fill=\"{}\" fill-opacity=\"{opacity}\" text-anchor=\"{anchor}\" xml:space=\"preserve\">{}</text>",
        esc(family).replace('\'', ""),
        fill.hex(),
        esc(text)
    ));
}

/// Where the logo goes (drawn onto the pixmap after the SVG is rendered).
struct LogoSlot {
    x: f32,
    y: f32,
    height: f32,
}

/// Build the card SVG (without the logo) and where to draw the logo.
fn layout(
    text: &CardText,
    style: &Style,
    fonts: &Fonts,
    logo: Option<&Logo>,
) -> (String, Option<LogoSlot>) {
    let title_face = fonts.title.measure();
    let body_face = fonts.body.measure();
    let (Some(title_face), Some(body_face)) = (title_face, body_face) else {
        return (String::new(), None);
    };
    let w = WIDTH as f32;
    let h = HEIGHT as f32;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH}\" height=\"{HEIGHT}\" viewBox=\"0 0 {WIDTH} {HEIGHT}\">"
    );
    let tf = fonts.title.family.as_str();
    let bf = fonts.body.family.as_str();
    let mut slot = None;
    match style.template {
        OgTemplate::Default => {
            let (bg, fg, accent) = (style.background, style.foreground, style.accent);
            svg.push_str(&format!(
                "<rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/><rect width=\"20\" height=\"{h}\" fill=\"{}\"/>",
                bg.hex(),
                accent.hex()
            ));
            let left = PAD + 20.0;
            let max_w = w - left - PAD;
            // Header: logo + header text.
            let mut x = left;
            if let Some(logo) = logo {
                slot = Some(LogoSlot {
                    x,
                    y: 72.0,
                    height: 56.0,
                });
                x += logo.width_at(56.0) + 20.0;
            }
            if !text.header.is_empty() {
                let (_, lines) = fit(&body_face, &text.header, &[32.0], left + max_w - x, 1);
                if let Some(line) = lines.first() {
                    text_el(&mut svg, x, 112.0, bf, 500, 32.0, fg, 0.75, "start", line);
                }
            }
            // Title (+ subtitle), vertically centered between header and footer.
            let (size, lines) = fit(
                &title_face,
                &text.title,
                &[84.0, 72.0, 60.0, 52.0],
                max_w,
                3,
            );
            let lh = size * 1.14;
            let sub = text
                .subtitle
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| fit(&body_face, s, &[34.0], max_w, 2).1)
                .unwrap_or_default();
            let sub_lh = 34.0 * 1.35;
            let block = lines.len() as f32 * lh
                + if sub.is_empty() {
                    0.0
                } else {
                    24.0 + sub.len() as f32 * sub_lh
                };
            let top = 160.0 + ((500.0 - 160.0) - block).max(0.0) / 2.0;
            for (i, line) in lines.iter().enumerate() {
                let y = top + size * 0.92 + i as f32 * lh;
                text_el(&mut svg, left, y, tf, 700, size, fg, 1.0, "start", line);
            }
            let sub_top = top + lines.len() as f32 * lh + 24.0;
            for (i, line) in sub.iter().enumerate() {
                let y = sub_top + 34.0 * 0.95 + i as f32 * sub_lh;
                text_el(&mut svg, left, y, bf, 500, 34.0, fg, 0.7, "start", line);
            }
            // Footer: meta left, accent rule right.
            if !text.meta.is_empty() {
                let (_, meta) = fit(&body_face, &text.meta, &[28.0], max_w - 140.0, 1);
                if let Some(line) = meta.first() {
                    text_el(
                        &mut svg, left, 566.0, bf, 500, 28.0, fg, 0.65, "start", line,
                    );
                }
            }
            svg.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"552\" width=\"96\" height=\"8\" rx=\"4\" fill=\"{}\"/>",
                w - PAD - 96.0,
                accent.hex()
            ));
        }
        OgTemplate::Minimal => {
            let (bg, fg, accent) = (style.background, style.foreground, style.accent);
            svg.push_str(&format!(
                "<rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
                bg.hex()
            ));
            let left = PAD;
            let max_w = w - 2.0 * PAD;
            if !text.meta.is_empty() {
                let (_, meta) = fit(&body_face, &text.meta, &[26.0], max_w, 1);
                if let Some(line) = meta.first() {
                    text_el(
                        &mut svg, left, 116.0, bf, 500, 26.0, accent, 1.0, "start", line,
                    );
                }
            }
            let (size, lines) = fit(
                &title_face,
                &text.title,
                &[72.0, 60.0, 52.0, 46.0],
                max_w,
                3,
            );
            let lh = size * 1.18;
            let top = if text.meta.is_empty() { 96.0 } else { 160.0 };
            for (i, line) in lines.iter().enumerate() {
                let y = top + size * 0.92 + i as f32 * lh;
                text_el(&mut svg, left, y, tf, 700, size, fg, 1.0, "start", line);
            }
            let mut bottom = top + lines.len() as f32 * lh;
            if let Some(sub) = text.subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
                let (_, sub) = fit(&body_face, sub, &[30.0], max_w, 2);
                for (i, line) in sub.iter().enumerate() {
                    let y = bottom + 20.0 + 30.0 * 0.95 + i as f32 * 30.0 * 1.35;
                    text_el(&mut svg, left, y, bf, 500, 30.0, fg, 0.65, "start", line);
                }
                bottom += 20.0 + sub.len() as f32 * 30.0 * 1.35;
            }
            let _ = bottom;
            svg.push_str(&format!(
                "<rect x=\"{left}\" y=\"494\" width=\"{max_w}\" height=\"2\" fill=\"{}\" fill-opacity=\"0.15\"/><rect x=\"{left}\" y=\"492\" width=\"72\" height=\"6\" fill=\"{}\"/>",
                fg.hex(),
                accent.hex()
            ));
            let mut x = left;
            if let Some(logo) = logo {
                slot = Some(LogoSlot {
                    x,
                    y: 528.0,
                    height: 40.0,
                });
                x += logo.width_at(40.0) + 16.0;
            }
            if !text.header.is_empty() {
                let (_, lines) = fit(&body_face, &text.header, &[28.0], left + max_w - x, 1);
                if let Some(line) = lines.first() {
                    text_el(&mut svg, x, 558.0, bf, 500, 28.0, fg, 0.8, "start", line);
                }
            }
        }
        OgTemplate::Bold => {
            let bg = style.accent;
            // Text color: whichever of the site's background/foreground (or
            // white/black) reads best on the accent.
            let ink = [
                style.background,
                style.foreground,
                Rgb(255, 255, 255),
                Rgb(17, 17, 17),
            ]
            .into_iter()
            .fold((Rgb(255, 255, 255), 0.0), |best, c| {
                let r = c.contrast(bg);
                if r > best.1 + 0.5 {
                    (c, r)
                } else {
                    best
                }
            })
            .0;
            svg.push_str(&format!(
                "<rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
                bg.hex()
            ));
            // Decorative rings in the corner.
            svg.push_str(&format!(
                "<circle cx=\"{w}\" cy=\"0\" r=\"220\" fill=\"none\" stroke=\"{0}\" stroke-opacity=\"0.14\" stroke-width=\"36\"/><circle cx=\"{w}\" cy=\"0\" r=\"330\" fill=\"none\" stroke=\"{0}\" stroke-opacity=\"0.08\" stroke-width=\"24\"/>",
                ink.hex()
            ));
            let left = PAD;
            let max_w = w - 2.0 * PAD - 60.0;
            if !text.meta.is_empty() {
                let (_, meta) = fit(&body_face, &text.meta, &[26.0], max_w, 1);
                if let Some(line) = meta.first() {
                    text_el(
                        &mut svg, left, 112.0, bf, 500, 26.0, ink, 0.8, "start", line,
                    );
                }
            }
            let (size, lines) = fit(
                &title_face,
                &text.title,
                &[96.0, 84.0, 70.0, 58.0],
                max_w,
                3,
            );
            let lh = size * 1.08;
            let sub = text
                .subtitle
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| fit(&body_face, s, &[32.0], max_w, 2).1)
                .unwrap_or_default();
            let block = lines.len() as f32 * lh
                + if sub.is_empty() {
                    0.0
                } else {
                    20.0 + sub.len() as f32 * 32.0 * 1.35
                };
            let top = 140.0 + ((480.0 - 140.0) - block).max(0.0) / 2.0;
            for (i, line) in lines.iter().enumerate() {
                let y = top + size * 0.92 + i as f32 * lh;
                text_el(&mut svg, left, y, tf, 700, size, ink, 1.0, "start", line);
            }
            let sub_top = top + lines.len() as f32 * lh + 20.0;
            for (i, line) in sub.iter().enumerate() {
                let y = sub_top + 32.0 * 0.95 + i as f32 * 32.0 * 1.35;
                text_el(&mut svg, left, y, bf, 500, 32.0, ink, 0.8, "start", line);
            }
            svg.push_str(&format!(
                "<rect x=\"0\" y=\"522\" width=\"{w}\" height=\"108\" fill=\"{}\" fill-opacity=\"0.12\"/>",
                ink.hex()
            ));
            let mut x = left;
            if let Some(logo) = logo {
                slot = Some(LogoSlot {
                    x,
                    y: 552.0,
                    height: 48.0,
                });
                x += logo.width_at(48.0) + 18.0;
            }
            if !text.header.is_empty() {
                let (_, lines) = fit(&body_face, &text.header, &[30.0], w - PAD - x, 1);
                if let Some(line) = lines.first() {
                    text_el(&mut svg, x, 587.0, bf, 500, 30.0, ink, 1.0, "start", line);
                }
            }
        }
    }
    svg.push_str("</svg>");
    (svg, slot)
}

/// Render a card to PNG bytes.
pub fn render(
    text: &CardText,
    style: &Style,
    fonts: &Fonts,
    logo: Option<&Logo>,
) -> Result<Vec<u8>, String> {
    let (svg, slot) = layout(text, style, fonts, logo);
    let opt = usvg::Options {
        fontdb: fonts.db.clone(),
        font_family: fonts.body.family.clone(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &opt).map_err(|e| format!("card SVG: {e}"))?;
    let mut pixmap =
        tiny_skia::Pixmap::new(WIDTH, HEIGHT).ok_or_else(|| "cannot allocate card".to_string())?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    if let (Some(logo), Some(slot)) = (logo, slot) {
        logo.draw(&mut pixmap, slot.x, slot.y, slot.height);
    }
    pixmap.encode_png().map_err(|e| format!("PNG encode: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face() -> Measure<'static> {
        Measure {
            font: FontRef::new(BUNDLED_TITLE).unwrap(),
        }
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!(Rgb::parse("#fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("#0057B7"), Some(Rgb(0, 0x57, 0xb7)));
        assert_eq!(Rgb::parse(" #000000 "), Some(Rgb(0, 0, 0)));
        for bad in ["fff", "#ffff", "#ggg", "red", "", "#12345", "rgb(0,0,0)"] {
            assert_eq!(Rgb::parse(bad), None, "{bad}");
        }
        assert_eq!(Rgb(0, 0x57, 0xb7).hex(), "#0057b7");
    }

    #[test]
    fn contrast_ratio_extremes() {
        let r = Rgb(0, 0, 0).contrast(Rgb(255, 255, 255));
        assert!((r - 21.0).abs() < 0.01);
        assert!((Rgb(10, 10, 10).contrast(Rgb(10, 10, 10)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn short_title_stays_at_largest_size_on_one_line() {
        let (size, lines) = fit(&face(), "Hello", &[76.0, 64.0, 54.0], 1000.0, 3);
        assert_eq!(size, 76.0);
        assert_eq!(lines, vec!["Hello".to_string()]);
    }

    #[test]
    fn wraps_and_shrinks_before_ellipsis() {
        let f = face();
        let text =
            "A fairly long title that needs to wrap across more than a single line of the card";
        let (size, lines) = fit(&f, text, &[76.0, 64.0, 54.0], 1000.0, 3);
        assert!(lines.len() > 1 && lines.len() <= 3);
        assert!(!lines.last().unwrap().ends_with('…'));
        for l in &lines {
            assert!(f.width(l, size) <= 1000.0, "{l}");
        }
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn overlong_title_is_ellipsized_to_three_lines() {
        let f = face();
        let text = "word ".repeat(200);
        let (size, lines) = fit(&f, &text, &[76.0, 64.0, 54.0], 1000.0, 3);
        assert_eq!(size, 54.0);
        assert_eq!(lines.len(), 3);
        assert!(lines[2].ends_with('…'));
        assert!(f.width(&lines[2], size) <= 1000.0);
    }

    #[test]
    fn overlong_word_is_broken() {
        let f = face();
        let text = "x".repeat(300);
        let lines = wrap(&f, &text, 76.0, 1000.0);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(f.width(l, 76.0) <= 1000.0);
        }
    }

    #[test]
    fn bundled_fonts_cover_latin_not_cjk() {
        let fonts = Fonts::load(None).unwrap();
        assert!(fonts.covers("Héllo, wörld — “quotes” & ½ Ελληνικά Кириллица"));
        assert!(!fonts.covers("你好"));
        assert!(!fonts.covers("emoji 🎉"));
    }

    #[test]
    fn bad_user_font_is_an_error() {
        assert!(Fonts::load(Some(b"not a font".to_vec())).is_err());
    }

    #[test]
    fn escapes_svg_text() {
        assert_eq!(esc("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn renders_png_of_card_size() {
        let fonts = Fonts::load(None).unwrap();
        for template in [OgTemplate::Default, OgTemplate::Minimal, OgTemplate::Bold] {
            let style = Style {
                template,
                background: Rgb(255, 255, 255),
                foreground: Rgb(17, 17, 17),
                accent: Rgb(0, 0x57, 0xb7),
            };
            let text = CardText {
                header: "My <Site>".into(),
                title: "Hello & welcome".into(),
                subtitle: Some("A description".into()),
                meta: "Posts · 2026-10-22".into(),
            };
            let png = render(&text, &style, &fonts, None).unwrap();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), WIDTH);
            assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), HEIGHT);
            // Rendering is deterministic.
            assert_eq!(png, render(&text, &style, &fonts, None).unwrap());
        }
    }
}
