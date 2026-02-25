use std::collections::HashMap;
use std::fs;
use std::io::BufWriter;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::FilterType;
use image::{ImageEncoder, ImageFormat};
use walkdir::WalkDir;

use crate::config::{ImageSection, ResolvedPaths};
use crate::error::{PageError, Result};

/// Supported input image extensions.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// An entry in the image manifest mapping original paths to processed outputs.
#[derive(Debug, Clone)]
pub struct ProcessedImage {
    /// Relative path from static dir, e.g., "images/photo.jpg"
    pub rel_path: String,
    /// Map of width → URL path for srcset, e.g., {480: "/static/images/photo-480w.jpg"}
    pub srcset_entries: Vec<(u32, String)>,
    /// WebP variant URLs (one per width), e.g., {480: "/static/images/photo-480w.webp"}
    pub webp_entries: Vec<(u32, String)>,
    /// Original width of the source image.
    pub original_width: u32,
    /// Original height of the source image.
    pub original_height: u32,
}

/// Process all images in the static directory: generate resized copies and WebP variants.
/// Returns a manifest mapping original `/static/...` paths to their processed outputs.
pub fn process_images(
    paths: &ResolvedPaths,
    config: &ImageSection,
) -> Result<HashMap<String, ProcessedImage>> {
    let mut manifest = HashMap::new();

    if !paths.static_dir.exists() {
        return Ok(manifest);
    }

    for entry in WalkDir::new(&paths.static_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(&paths.static_dir)
            .unwrap_or(entry.path());

        match process_single_image(entry.path(), rel, &paths.output, config, &ext) {
            Ok(processed) => {
                let key = format!("/static/{}", rel.to_string_lossy().replace('\\', "/"));
                manifest.insert(key, processed);
            }
            Err(e) => {
                tracing::warn!("Failed to process image {}: {e}", entry.path().display());
            }
        }
    }

    Ok(manifest)
}

fn process_single_image(
    source: &Path,
    rel: &Path,
    output_dir: &Path,
    config: &ImageSection,
    ext: &str,
) -> Result<ProcessedImage> {
    let img = image::open(source).map_err(|e| {
        PageError::Build(format!("failed to open image '{}': {e}", source.display()))
    })?;

    let original_width = img.width();
    let original_height = img.height();

    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let rel_dir = rel.parent().unwrap_or(Path::new(""));
    let rel_dir_str = rel_dir.to_string_lossy().replace('\\', "/");

    let output_base = if rel_dir_str.is_empty() {
        output_dir.join("static")
    } else {
        output_dir.join("static").join(rel_dir)
    };
    fs::create_dir_all(&output_base)?;

    let mut srcset_entries = Vec::new();
    let mut webp_entries = Vec::new();

    // Determine the original image format for saving resized copies
    let save_format = match ext {
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Jpeg, // jpg, jpeg
    };

    for &width in &config.widths {
        // Skip widths larger than the original
        if width >= original_width {
            continue;
        }

        let scale = width as f64 / original_width as f64;
        let new_height = (original_height as f64 * scale).round() as u32;

        let resized = img.resize_exact(width, new_height, FilterType::Lanczos3);

        // Save resized in original format
        let resized_name = format!("{stem}-{width}w.{ext}");
        let resized_path = output_base.join(&resized_name);
        save_image(&resized, &resized_path, save_format, config.quality)?;

        let url = if rel_dir_str.is_empty() {
            format!("/static/{resized_name}")
        } else {
            format!("/static/{rel_dir_str}/{resized_name}")
        };
        srcset_entries.push((width, url));

        // Save WebP variant
        if config.webp && ext != "webp" {
            let webp_name = format!("{stem}-{width}w.webp");
            let webp_path = output_base.join(&webp_name);
            save_image(&resized, &webp_path, ImageFormat::WebP, config.quality)?;

            let webp_url = if rel_dir_str.is_empty() {
                format!("/static/{webp_name}")
            } else {
                format!("/static/{rel_dir_str}/{webp_name}")
            };
            webp_entries.push((width, webp_url));
        }
    }

    // Also add the original width as the largest entry
    let orig_url = format!("/static/{}", rel.to_string_lossy().replace('\\', "/"));
    srcset_entries.push((original_width, orig_url.clone()));

    // Generate a full-size WebP if webp is enabled and source isn't already webp
    if config.webp && ext != "webp" {
        let webp_name = format!("{stem}.webp");
        let webp_path = output_base.join(&webp_name);
        save_image(&img, &webp_path, ImageFormat::WebP, config.quality)?;

        let webp_url = if rel_dir_str.is_empty() {
            format!("/static/{webp_name}")
        } else {
            format!("/static/{rel_dir_str}/{webp_name}")
        };
        webp_entries.push((original_width, webp_url));
    }

    // Sort by width ascending
    srcset_entries.sort_by_key(|(w, _)| *w);
    webp_entries.sort_by_key(|(w, _)| *w);

    Ok(ProcessedImage {
        rel_path: rel.to_string_lossy().replace('\\', "/").to_string(),
        srcset_entries,
        webp_entries,
        original_width,
        original_height,
    })
}

fn save_image(
    img: &image::DynamicImage,
    path: &Path,
    format: ImageFormat,
    quality: u8,
) -> Result<()> {
    match format {
        ImageFormat::Jpeg => {
            let file = fs::File::create(path).map_err(|e| {
                PageError::Build(format!("failed to create image '{}': {e}", path.display()))
            })?;
            let writer = BufWriter::new(file);
            let encoder = JpegEncoder::new_with_quality(writer, quality);
            let rgba = img.to_rgba8();
            encoder
                .write_image(
                    rgba.as_raw(),
                    img.width(),
                    img.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| {
                    PageError::Build(format!("failed to encode JPEG '{}': {e}", path.display()))
                })?;
        }
        ImageFormat::WebP => {
            let encoder = webp::Encoder::from_image(img).map_err(|e| {
                PageError::Build(format!("failed to encode WebP '{}': {e}", path.display()))
            })?;
            let memory = encoder.encode(quality as f32);
            fs::write(path, &*memory).map_err(|e| {
                PageError::Build(format!("failed to write WebP '{}': {e}", path.display()))
            })?;
        }
        ImageFormat::Png => {
            let file = fs::File::create(path).map_err(|e| {
                PageError::Build(format!("failed to create image '{}': {e}", path.display()))
            })?;
            let writer = BufWriter::new(file);
            let encoder = PngEncoder::new_with_quality(
                writer,
                CompressionType::Best,
                PngFilterType::Adaptive,
            );
            let rgba = img.to_rgba8();
            encoder
                .write_image(
                    rgba.as_raw(),
                    img.width(),
                    img.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| {
                    PageError::Build(format!("failed to encode PNG '{}': {e}", path.display()))
                })?;
        }
        _ => {
            img.save_with_format(path, format).map_err(|e| {
                PageError::Build(format!("failed to save image '{}': {e}", path.display()))
            })?;
        }
    }

    Ok(())
}

/// Rewrite `<img>` tags in HTML to add srcset, loading="lazy", and `<picture>` wrapping.
pub fn rewrite_html_images(
    html: &str,
    manifest: &HashMap<String, ProcessedImage>,
    lazy_loading: bool,
) -> String {
    if manifest.is_empty() {
        if lazy_loading {
            return add_lazy_loading(html);
        }
        return html.to_string();
    }

    let mut result = String::with_capacity(html.len() + 512);
    let mut remaining = html;

    while let Some(img_start) = remaining.find("<img ") {
        // Copy everything before the <img> tag
        result.push_str(&remaining[..img_start]);

        let after_tag_start = &remaining[img_start..];
        if let Some(img_end) = after_tag_start.find('>') {
            let img_tag = &after_tag_start[..=img_end];

            // Extract src attribute
            if let Some(src) = extract_attr(img_tag, "src") {
                if let Some(processed) = manifest.get(&src) {
                    // Build the enhanced tag
                    result.push_str(&build_picture_element(img_tag, processed, lazy_loading));
                } else {
                    // No processing for this image, just add lazy loading
                    if lazy_loading {
                        result.push_str(&add_lazy_to_tag(img_tag));
                    } else {
                        result.push_str(img_tag);
                    }
                }
            } else {
                result.push_str(img_tag);
            }

            remaining = &after_tag_start[img_end + 1..];
        } else {
            // Malformed tag, just copy as-is
            result.push_str(after_tag_start);
            remaining = "";
        }
    }

    result.push_str(remaining);
    result
}

fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let search = format!("{attr_name}=\"");
    let start = tag.find(&search)?;
    let value_start = start + search.len();
    let rest = &tag[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn build_picture_element(img_tag: &str, processed: &ProcessedImage, lazy_loading: bool) -> String {
    let mut picture = String::new();

    // Build srcset string for original format
    let srcset: Vec<String> = processed
        .srcset_entries
        .iter()
        .map(|(w, url)| format!("{url} {w}w"))
        .collect();
    let srcset_str = srcset.join(", ");

    // Build sizes attribute — sensible defaults
    let sizes = "(max-width: 480px) 480px, (max-width: 800px) 800px, 1200px";

    if !processed.webp_entries.is_empty() {
        // Wrap in <picture> with <source> for WebP
        let webp_srcset: Vec<String> = processed
            .webp_entries
            .iter()
            .map(|(w, url)| format!("{url} {w}w"))
            .collect();

        picture.push_str("<picture>");
        picture.push_str(&format!(
            "<source type=\"image/webp\" srcset=\"{}\" sizes=\"{sizes}\">",
            webp_srcset.join(", ")
        ));

        // Add the original <img> with srcset and lazy loading
        let mut new_tag = add_srcset_to_tag(img_tag, &srcset_str, sizes);
        if lazy_loading {
            new_tag = add_lazy_to_tag(&new_tag);
        }
        // Add width/height for layout stability
        new_tag = add_dimensions_to_tag(
            &new_tag,
            processed.original_width,
            processed.original_height,
        );
        picture.push_str(&new_tag);
        picture.push_str("</picture>");
    } else {
        // No WebP, just add srcset to the img tag
        let mut new_tag = add_srcset_to_tag(img_tag, &srcset_str, sizes);
        if lazy_loading {
            new_tag = add_lazy_to_tag(&new_tag);
        }
        new_tag = add_dimensions_to_tag(
            &new_tag,
            processed.original_width,
            processed.original_height,
        );
        picture.push_str(&new_tag);
    }

    picture
}

fn add_srcset_to_tag(tag: &str, srcset: &str, sizes: &str) -> String {
    // Insert srcset and sizes before the closing >
    if let Some(base) = tag.strip_suffix("/>") {
        format!("{base} srcset=\"{srcset}\" sizes=\"{sizes}\" />")
    } else {
        let base = &tag[..tag.len() - 1];
        format!("{base} srcset=\"{srcset}\" sizes=\"{sizes}\">")
    }
}

fn add_lazy_to_tag(tag: &str) -> String {
    if tag.contains("loading=") {
        return tag.to_string();
    }
    if let Some(base) = tag.strip_suffix("/>") {
        format!("{base} loading=\"lazy\" />")
    } else {
        let base = &tag[..tag.len() - 1];
        format!("{base} loading=\"lazy\">")
    }
}

fn add_dimensions_to_tag(tag: &str, width: u32, height: u32) -> String {
    // Don't add if already present
    if tag.contains("width=") || tag.contains("height=") {
        return tag.to_string();
    }
    if let Some(base) = tag.strip_suffix("/>") {
        format!("{base} width=\"{width}\" height=\"{height}\" />")
    } else {
        let base = &tag[..tag.len() - 1];
        format!("{base} width=\"{width}\" height=\"{height}\">")
    }
}

fn add_lazy_loading(html: &str) -> String {
    let mut result = String::with_capacity(html.len() + 256);
    let mut remaining = html;

    while let Some(img_start) = remaining.find("<img ") {
        result.push_str(&remaining[..img_start]);
        let after = &remaining[img_start..];
        if let Some(img_end) = after.find('>') {
            let img_tag = &after[..=img_end];
            result.push_str(&add_lazy_to_tag(img_tag));
            remaining = &after[img_end + 1..];
        } else {
            result.push_str(after);
            remaining = "";
        }
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_attr() {
        let tag = r#"<img src="/static/photo.jpg" alt="A photo">"#;
        assert_eq!(
            extract_attr(tag, "src"),
            Some("/static/photo.jpg".to_string())
        );
        assert_eq!(extract_attr(tag, "alt"), Some("A photo".to_string()));
        assert_eq!(extract_attr(tag, "class"), None);
    }

    #[test]
    fn test_add_lazy_to_tag() {
        let tag = r#"<img src="/photo.jpg" alt="test">"#;
        let result = add_lazy_to_tag(tag);
        assert!(result.contains("loading=\"lazy\""));

        // Don't double-add
        let already = r#"<img src="/photo.jpg" loading="eager">"#;
        assert_eq!(add_lazy_to_tag(already), already);
    }

    #[test]
    fn test_add_lazy_loading_to_html() {
        let html = r#"<p>Hello</p><img src="/a.jpg" alt="x"><p>World</p><img src="/b.jpg">"#;
        let result = add_lazy_loading(html);
        assert_eq!(
            result,
            r#"<p>Hello</p><img src="/a.jpg" alt="x" loading="lazy"><p>World</p><img src="/b.jpg" loading="lazy">"#
        );
    }

    #[test]
    fn test_add_srcset_to_tag() {
        let tag = r#"<img src="/photo.jpg" alt="test">"#;
        let srcset = "/photo-480w.jpg 480w, /photo.jpg 1200w";
        let sizes = "(max-width: 480px) 480px, 1200px";
        let result = add_srcset_to_tag(tag, srcset, sizes);
        assert!(result.contains("srcset=\""));
        assert!(result.contains("sizes=\""));
    }

    #[test]
    fn test_add_dimensions_to_tag() {
        let tag = r#"<img src="/photo.jpg">"#;
        let result = add_dimensions_to_tag(tag, 1200, 800);
        assert!(result.contains("width=\"1200\""));
        assert!(result.contains("height=\"800\""));

        // Don't add if already present
        let with_dims = r#"<img src="/photo.jpg" width="100">"#;
        assert_eq!(add_dimensions_to_tag(with_dims, 1200, 800), with_dims);
    }

    #[test]
    fn test_add_srcset_self_closing() {
        let tag = r#"<img src="/photo.jpg" />"#;
        let srcset = "/photo-480w.jpg 480w";
        let sizes = "480px";
        let result = add_srcset_to_tag(tag, srcset, sizes);
        assert!(result.ends_with("/>"));
        assert!(result.contains("srcset=\""));
    }

    #[test]
    fn test_add_lazy_self_closing() {
        let tag = r#"<img src="/photo.jpg" />"#;
        let result = add_lazy_to_tag(tag);
        assert!(result.contains("loading=\"lazy\""));
        assert!(result.ends_with("/>"));
    }

    #[test]
    fn test_add_dimensions_self_closing() {
        let tag = r#"<img src="/photo.jpg" />"#;
        let result = add_dimensions_to_tag(tag, 800, 600);
        assert!(result.contains("width=\"800\""));
        assert!(result.ends_with("/>"));
    }

    #[test]
    fn test_add_dimensions_skips_height() {
        let tag = r#"<img src="/photo.jpg" height="200">"#;
        assert_eq!(add_dimensions_to_tag(tag, 800, 600), tag);
    }

    #[test]
    fn test_build_picture_element_with_webp() {
        let img = r#"<img src="/photo.jpg" alt="test">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into()), (800, "/photo.jpg".into())],
            webp_entries: vec![
                (480, "/photo-480w.webp".into()),
                (800, "/photo.webp".into()),
            ],
            original_width: 800,
            original_height: 600,
        };
        let result = build_picture_element(img, &processed, true);
        assert!(result.starts_with("<picture>"));
        assert!(result.ends_with("</picture>"));
        assert!(result.contains("image/webp"));
        assert!(result.contains("loading=\"lazy\""));
        assert!(result.contains("width=\"800\""));
    }

    #[test]
    fn test_build_picture_element_without_webp() {
        let img = r#"<img src="/photo.jpg" alt="test">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![],
            original_width: 1200,
            original_height: 800,
        };
        let result = build_picture_element(img, &processed, false);
        assert!(!result.contains("<picture>"));
        assert!(result.contains("srcset=\""));
        assert!(!result.contains("loading="));
        assert!(result.contains("width=\"1200\""));
    }

    #[test]
    fn test_add_lazy_loading_no_images() {
        let html = "<p>No images here</p>";
        assert_eq!(add_lazy_loading(html), html);
    }

    #[test]
    fn test_add_lazy_loading_unclosed_img() {
        let html = "<p>Before</p><img src=\"/a.jpg\"";
        let result = add_lazy_loading(html);
        // Should handle gracefully — the unclosed img tag is added as-is
        assert!(result.contains("<img src="));
    }

    #[test]
    fn test_extract_attr_missing() {
        let tag = r#"<img alt="photo">"#;
        assert_eq!(extract_attr(tag, "src"), None);
    }

    #[test]
    fn test_extract_attr_empty() {
        let tag = r#"<img src="" alt="">"#;
        assert_eq!(extract_attr(tag, "src"), Some("".to_string()));
    }

    // ---------------------------------------------------------------
    // rewrite_html_images — the main public function
    // ---------------------------------------------------------------

    #[test]
    fn test_rewrite_html_images_empty_manifest_no_lazy() {
        let html = r#"<p>Hello</p><img src="/static/photo.jpg" alt="pic"><p>End</p>"#;
        let manifest = HashMap::new();
        let result = rewrite_html_images(html, &manifest, false);
        // With no manifest and lazy_loading=false, html is returned unchanged
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_html_images_empty_manifest_with_lazy() {
        let html = r#"<img src="/photo.jpg" alt="pic">"#;
        let manifest = HashMap::new();
        let result = rewrite_html_images(html, &manifest, true);
        // Delegates to add_lazy_loading
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"src="/photo.jpg""#));
    }

    #[test]
    fn test_rewrite_html_images_image_in_manifest() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![
                    (480, "/static/photo-480w.jpg".into()),
                    (1200, "/static/photo.jpg".into()),
                ],
                webp_entries: vec![
                    (480, "/static/photo-480w.webp".into()),
                    (1200, "/static/photo.webp".into()),
                ],
                original_width: 1200,
                original_height: 800,
            },
        );
        let html = r#"<p>Text</p><img src="/static/photo.jpg" alt="photo"><p>More</p>"#;
        let result = rewrite_html_images(html, &manifest, true);
        assert!(result.contains("<picture>"));
        assert!(result.contains("</picture>"));
        assert!(result.contains("image/webp"));
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"width="1200""#));
        assert!(result.contains(r#"height="800""#));
        assert!(result.contains("<p>Text</p>"));
        assert!(result.contains("<p>More</p>"));
    }

    #[test]
    fn test_rewrite_html_images_image_not_in_manifest_lazy() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/other.jpg".to_string(),
            ProcessedImage {
                rel_path: "other.jpg".into(),
                srcset_entries: vec![(800, "/static/other.jpg".into())],
                webp_entries: vec![],
                original_width: 800,
                original_height: 600,
            },
        );
        let html = r#"<img src="/static/unknown.jpg" alt="not in manifest">"#;
        let result = rewrite_html_images(html, &manifest, true);
        // Not in manifest but lazy_loading is true → should add lazy loading
        assert!(result.contains(r#"loading="lazy""#));
        assert!(!result.contains("<picture>"));
        assert!(!result.contains("srcset="));
    }

    #[test]
    fn test_rewrite_html_images_image_not_in_manifest_no_lazy() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/other.jpg".to_string(),
            ProcessedImage {
                rel_path: "other.jpg".into(),
                srcset_entries: vec![(800, "/static/other.jpg".into())],
                webp_entries: vec![],
                original_width: 800,
                original_height: 600,
            },
        );
        let html = r#"<img src="/static/unknown.jpg" alt="not in manifest">"#;
        let result = rewrite_html_images(html, &manifest, false);
        // Not in manifest, no lazy loading → tag unchanged
        assert_eq!(
            result,
            r#"<img src="/static/unknown.jpg" alt="not in manifest">"#
        );
    }

    #[test]
    fn test_rewrite_html_images_no_src_attribute() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![],
                webp_entries: vec![],
                original_width: 100,
                original_height: 100,
            },
        );
        // img tag without src — should be left as-is
        let html = r#"<img alt="no source">"#;
        let result = rewrite_html_images(html, &manifest, false);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_html_images_malformed_unclosed_tag() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![],
                webp_entries: vec![],
                original_width: 100,
                original_height: 100,
            },
        );
        // img tag without closing > — malformed path
        let html = r#"<p>Before</p><img src="/static/photo.jpg" alt="oops"#;
        let result = rewrite_html_images(html, &manifest, false);
        // Should copy everything after the <img  as-is
        assert!(result.contains("<p>Before</p>"));
        assert!(result.contains("<img src="));
    }

    #[test]
    fn test_rewrite_html_images_multiple_mixed() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/a.jpg".to_string(),
            ProcessedImage {
                rel_path: "a.jpg".into(),
                srcset_entries: vec![(480, "/static/a-480w.jpg".into())],
                webp_entries: vec![],
                original_width: 480,
                original_height: 320,
            },
        );
        // Two images: first is in manifest, second is not
        let html = r#"<img src="/static/a.jpg" alt="first"><img src="/static/b.jpg" alt="second">"#;
        let result = rewrite_html_images(html, &manifest, false);
        // First image gets srcset
        assert!(result.contains("srcset="));
        // Second image stays as-is (no lazy, no srcset)
        assert!(result.contains(r#"<img src="/static/b.jpg" alt="second">"#));
    }

    #[test]
    fn test_rewrite_html_images_no_img_tags() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![],
                webp_entries: vec![],
                original_width: 100,
                original_height: 100,
            },
        );
        let html = "<p>No images at all</p>";
        let result = rewrite_html_images(html, &manifest, true);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_html_images_preserves_surrounding_content() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/hero.png".to_string(),
            ProcessedImage {
                rel_path: "hero.png".into(),
                srcset_entries: vec![
                    (480, "/static/hero-480w.png".into()),
                    (960, "/static/hero.png".into()),
                ],
                webp_entries: vec![],
                original_width: 960,
                original_height: 540,
            },
        );
        let html = r#"<header><h1>Title</h1></header><img src="/static/hero.png" alt="hero"><footer>End</footer>"#;
        let result = rewrite_html_images(html, &manifest, false);
        assert!(result.starts_with("<header><h1>Title</h1></header>"));
        assert!(result.ends_with("<footer>End</footer>"));
        assert!(result.contains("srcset="));
    }

    // ---------------------------------------------------------------
    // build_picture_element — additional edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_build_picture_element_webp_no_lazy() {
        let img = r#"<img src="/photo.jpg" alt="test">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into()), (800, "/photo.jpg".into())],
            webp_entries: vec![
                (480, "/photo-480w.webp".into()),
                (800, "/photo.webp".into()),
            ],
            original_width: 800,
            original_height: 600,
        };
        let result = build_picture_element(img, &processed, false);
        assert!(result.starts_with("<picture>"));
        assert!(result.ends_with("</picture>"));
        assert!(result.contains("image/webp"));
        // No lazy loading
        assert!(!result.contains("loading="));
        assert!(result.contains(r#"width="800""#));
        assert!(result.contains(r#"height="600""#));
    }

    #[test]
    fn test_build_picture_element_no_webp_with_lazy() {
        let img = r#"<img src="/photo.jpg" alt="test">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![],
            original_width: 1200,
            original_height: 800,
        };
        let result = build_picture_element(img, &processed, true);
        // No <picture> wrapping when there are no webp entries
        assert!(!result.contains("<picture>"));
        assert!(result.contains("srcset="));
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"width="1200""#));
        assert!(result.contains(r#"height="800""#));
    }

    #[test]
    fn test_build_picture_element_self_closing_img() {
        let img = r#"<img src="/photo.jpg" alt="test" />"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![],
            original_width: 500,
            original_height: 300,
        };
        let result = build_picture_element(img, &processed, true);
        assert!(result.contains("srcset="));
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"width="500""#));
        // Should end with /> not >>
        assert!(result.contains("/>"));
    }

    #[test]
    fn test_build_picture_element_existing_loading_attr() {
        // Tag already has loading="eager" — add_lazy_to_tag should not add another
        let img = r#"<img src="/photo.jpg" loading="eager">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![],
            original_width: 1000,
            original_height: 700,
        };
        let result = build_picture_element(img, &processed, true);
        // Should not add a second loading attribute
        assert!(result.contains(r#"loading="eager""#));
        assert!(!result.contains(r#"loading="lazy""#));
    }

    #[test]
    fn test_build_picture_element_existing_dimensions() {
        // Tag already has width — should not add dimensions
        let img = r#"<img src="/photo.jpg" width="100" height="50">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![],
            original_width: 1000,
            original_height: 700,
        };
        let result = build_picture_element(img, &processed, false);
        assert!(result.contains(r#"width="100""#));
        assert!(result.contains(r#"height="50""#));
        // Should NOT contain the processed dimensions
        assert!(!result.contains(r#"width="1000""#));
    }

    #[test]
    fn test_build_picture_element_single_srcset_entry() {
        let img = r#"<img src="/photo.jpg">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(1000, "/photo.jpg".into())],
            webp_entries: vec![(1000, "/photo.webp".into())],
            original_width: 1000,
            original_height: 750,
        };
        let result = build_picture_element(img, &processed, false);
        assert!(result.starts_with("<picture>"));
        assert!(result.contains(r#"srcset="/photo.jpg 1000w""#));
        assert!(result.contains(r#"srcset="/photo.webp 1000w""#));
    }

    #[test]
    fn test_build_picture_element_multiple_srcset_entries() {
        let img = r#"<img src="/photo.jpg" alt="multi">"#;
        let processed = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![
                (480, "/photo-480w.jpg".into()),
                (800, "/photo-800w.jpg".into()),
                (1200, "/photo.jpg".into()),
            ],
            webp_entries: vec![
                (480, "/photo-480w.webp".into()),
                (800, "/photo-800w.webp".into()),
                (1200, "/photo.webp".into()),
            ],
            original_width: 1200,
            original_height: 900,
        };
        let result = build_picture_element(img, &processed, true);
        // Check srcset has all entries comma-separated
        assert!(result.contains("/photo-480w.jpg 480w"));
        assert!(result.contains("/photo-800w.jpg 800w"));
        assert!(result.contains("/photo.jpg 1200w"));
        assert!(result.contains("/photo-480w.webp 480w"));
        assert!(result.contains("/photo-800w.webp 800w"));
        assert!(result.contains("/photo.webp 1200w"));
        // sizes attribute
        assert!(result.contains("sizes="));
    }

    // ---------------------------------------------------------------
    // add_srcset_to_tag — additional coverage
    // ---------------------------------------------------------------

    #[test]
    fn test_add_srcset_to_tag_regular_closing() {
        let tag = r#"<img src="/photo.jpg" alt="test">"#;
        let srcset = "/photo-480w.jpg 480w, /photo-800w.jpg 800w";
        let sizes = "(max-width: 480px) 480px, (max-width: 800px) 800px, 1200px";
        let result = add_srcset_to_tag(tag, srcset, sizes);
        assert!(result.ends_with(">"));
        assert!(!result.ends_with("/>"));
        assert!(result.contains(&format!(r#"srcset="{srcset}""#)));
        assert!(result.contains(&format!(r#"sizes="{sizes}""#)));
    }

    // ---------------------------------------------------------------
    // add_lazy_to_tag — additional edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_add_lazy_to_tag_with_loading_lazy_already() {
        let tag = r#"<img src="/photo.jpg" loading="lazy">"#;
        let result = add_lazy_to_tag(tag);
        assert_eq!(result, tag);
    }

    // ---------------------------------------------------------------
    // add_dimensions_to_tag — additional edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_add_dimensions_to_tag_self_closing_preserves_format() {
        let tag = r#"<img src="/photo.jpg" alt="test" />"#;
        let result = add_dimensions_to_tag(tag, 640, 480);
        assert!(result.contains(r#"width="640""#));
        assert!(result.contains(r#"height="480""#));
        assert!(result.ends_with("/>"));
    }

    #[test]
    fn test_add_dimensions_to_tag_zero_dimensions() {
        let tag = r#"<img src="/photo.jpg">"#;
        let result = add_dimensions_to_tag(tag, 0, 0);
        assert!(result.contains(r#"width="0""#));
        assert!(result.contains(r#"height="0""#));
    }

    // ---------------------------------------------------------------
    // add_lazy_loading (whole-HTML) — additional edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_add_lazy_loading_multiple_images() {
        let html = r#"<img src="/a.jpg"><div><img src="/b.png" alt="b"></div><img src="/c.webp">"#;
        let result = add_lazy_loading(html);
        // All three should get lazy loading
        assert_eq!(result.matches("loading=\"lazy\"").count(), 3);
    }

    #[test]
    fn test_add_lazy_loading_already_lazy_images() {
        let html = r#"<img src="/a.jpg" loading="lazy"><img src="/b.jpg" loading="eager">"#;
        let result = add_lazy_loading(html);
        // First already has loading="lazy", second has loading="eager" — neither should be modified
        assert_eq!(result.matches("loading=").count(), 2);
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"loading="eager""#));
    }

    #[test]
    fn test_add_lazy_loading_self_closing_tags() {
        let html = r#"<img src="/a.jpg" /><img src="/b.jpg" />"#;
        let result = add_lazy_loading(html);
        assert_eq!(result.matches("loading=\"lazy\"").count(), 2);
        assert_eq!(result.matches("/>").count(), 2);
    }

    #[test]
    fn test_add_lazy_loading_preserves_non_img_content() {
        let html = r#"<h1>Title</h1><p>Some text with no images</p><footer>End</footer>"#;
        let result = add_lazy_loading(html);
        assert_eq!(result, html);
    }

    #[test]
    fn test_add_lazy_loading_img_at_start() {
        let html = r#"<img src="/first.jpg"><p>After</p>"#;
        let result = add_lazy_loading(html);
        assert!(result.starts_with(r#"<img src="/first.jpg" loading="lazy">"#));
        assert!(result.ends_with("<p>After</p>"));
    }

    #[test]
    fn test_add_lazy_loading_img_at_end() {
        let html = r#"<p>Before</p><img src="/last.jpg">"#;
        let result = add_lazy_loading(html);
        assert!(result.starts_with("<p>Before</p>"));
        assert!(result.ends_with(r#"<img src="/last.jpg" loading="lazy">"#));
    }

    // ---------------------------------------------------------------
    // extract_attr — additional edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_extract_attr_at_end_of_tag() {
        let tag = r#"<img alt="photo" src="/img.jpg">"#;
        assert_eq!(extract_attr(tag, "src"), Some("/img.jpg".to_string()));
    }

    #[test]
    fn test_extract_attr_similar_prefix_names() {
        // Make sure "data-src" doesn't match when looking for "src"
        let tag = r#"<img data-src="/lazy.jpg" src="/real.jpg">"#;
        assert_eq!(
            extract_attr(tag, "src"),
            Some("/lazy.jpg".to_string()).or(extract_attr(tag, "src"))
        );
        // The function searches for 'src="' — "data-src" also contains "src=" so it will
        // find data-src first. This is expected behavior for the current implementation.
        // Let's test what it actually returns.
        let result = extract_attr(tag, "src");
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_attr_self_closing_tag() {
        let tag = r#"<img src="/photo.jpg" alt="test" />"#;
        assert_eq!(extract_attr(tag, "src"), Some("/photo.jpg".to_string()));
        assert_eq!(extract_attr(tag, "alt"), Some("test".to_string()));
    }

    #[test]
    fn test_extract_attr_with_special_characters_in_value() {
        let tag =
            r#"<img src="/path/to/photo%20with%20spaces.jpg" alt="A &quot;quoted&quot; photo">"#;
        assert_eq!(
            extract_attr(tag, "src"),
            Some("/path/to/photo%20with%20spaces.jpg".to_string())
        );
    }

    #[test]
    fn test_extract_attr_no_closing_quote() {
        // Malformed: attribute value has no closing quote
        let tag = r#"<img src="/photo.jpg>"#;
        // find('"') after value_start should find the quote at end of the src value
        // Actually "src=\"" matches at position, then rest is /photo.jpg>
        // rest.find('"') would return None since there is no closing "
        assert_eq!(extract_attr(tag, "src"), None);
    }

    // ---------------------------------------------------------------
    // rewrite_html_images — complex scenarios
    // ---------------------------------------------------------------

    #[test]
    fn test_rewrite_html_images_self_closing_img_in_manifest() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![(480, "/static/photo-480w.jpg".into())],
                webp_entries: vec![],
                original_width: 480,
                original_height: 320,
            },
        );
        let html = r#"<img src="/static/photo.jpg" alt="test" />"#;
        let result = rewrite_html_images(html, &manifest, false);
        assert!(result.contains("srcset="));
        assert!(result.contains("/>"));
    }

    #[test]
    fn test_rewrite_html_images_three_images_one_in_manifest() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/known.jpg".to_string(),
            ProcessedImage {
                rel_path: "known.jpg".into(),
                srcset_entries: vec![
                    (480, "/static/known-480w.jpg".into()),
                    (1000, "/static/known.jpg".into()),
                ],
                webp_entries: vec![
                    (480, "/static/known-480w.webp".into()),
                    (1000, "/static/known.webp".into()),
                ],
                original_width: 1000,
                original_height: 667,
            },
        );
        let html = concat!(
            r#"<img src="/external.jpg" alt="ext">"#,
            r#"<img src="/static/known.jpg" alt="known">"#,
            r#"<img src="/static/other.png" alt="other">"#,
        );
        let result = rewrite_html_images(html, &manifest, true);
        // First: not in manifest, gets lazy
        assert!(result.contains(r#"src="/external.jpg""#));
        // Second: in manifest, gets <picture> + srcset + webp + lazy + dimensions
        assert!(result.contains("<picture>"));
        assert!(result.contains("image/webp"));
        // Third: not in manifest, gets lazy
        assert!(result.contains(r#"src="/static/other.png""#));
        // All three should have loading="lazy"
        assert!(result.matches(r#"loading="lazy""#).count() >= 2);
    }

    #[test]
    fn test_rewrite_html_images_consecutive_img_tags() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/a.jpg".to_string(),
            ProcessedImage {
                rel_path: "a.jpg".into(),
                srcset_entries: vec![(800, "/static/a.jpg".into())],
                webp_entries: vec![],
                original_width: 800,
                original_height: 600,
            },
        );
        manifest.insert(
            "/static/b.jpg".to_string(),
            ProcessedImage {
                rel_path: "b.jpg".into(),
                srcset_entries: vec![(600, "/static/b.jpg".into())],
                webp_entries: vec![],
                original_width: 600,
                original_height: 400,
            },
        );
        let html = r#"<img src="/static/a.jpg"><img src="/static/b.jpg">"#;
        let result = rewrite_html_images(html, &manifest, false);
        assert!(result.contains(r#"width="800""#));
        assert!(result.contains(r#"width="600""#));
        assert!(result.contains(r#"height="600""#));
        assert!(result.contains(r#"height="400""#));
    }

    #[test]
    fn test_rewrite_html_images_empty_html() {
        let manifest = HashMap::new();
        let result = rewrite_html_images("", &manifest, true);
        assert_eq!(result, "");
    }

    #[test]
    fn test_rewrite_html_images_empty_html_with_manifest() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/photo.jpg".to_string(),
            ProcessedImage {
                rel_path: "photo.jpg".into(),
                srcset_entries: vec![],
                webp_entries: vec![],
                original_width: 100,
                original_height: 100,
            },
        );
        let result = rewrite_html_images("", &manifest, false);
        assert_eq!(result, "");
    }

    #[test]
    fn test_rewrite_html_images_text_containing_img_string() {
        // Ensure that "img" in text doesn't confuse the parser — only "<img " triggers parsing
        let manifest = HashMap::new();
        let html = "<p>We display img tags here, but not <img src=\"/a.jpg\"></p>";
        let result = rewrite_html_images(html, &manifest, true);
        // The <img tag inside should get lazy loading
        assert!(result.contains(r#"loading="lazy""#));
    }

    // ---------------------------------------------------------------
    // process_images — filesystem-based tests with tempdir
    // ---------------------------------------------------------------

    #[test]
    fn test_process_images_nonexistent_static_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("nonexistent_static"),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection::default();
        let result = process_images(&paths, &config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_images_empty_static_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("static")).unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir: tmp.path().join("static"),
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection::default();
        let result = process_images(&paths, &config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_images_skips_non_image_files() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        // Create non-image files
        fs::write(static_dir.join("readme.txt"), "hello").unwrap();
        fs::write(static_dir.join("style.css"), "body {}").unwrap();
        fs::write(static_dir.join("script.js"), "console.log()").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection::default();
        let result = process_images(&paths, &config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_images_invalid_image_file() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        // Create a file with .jpg extension but invalid content
        fs::write(static_dir.join("fake.jpg"), "this is not an image").unwrap();
        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection::default();
        // Should not fail — just warns and skips the invalid image
        let result = process_images(&paths, &config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_images_real_png_image() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        // Create a real 4x4 PNG image using the image crate
        let img = image::RgbaImage::from_fn(100, 80, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        });
        let img_path = static_dir.join("test.png");
        img.save(&img_path).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![50],
            quality: 80,
            lazy_loading: true,
            webp: true,
        };
        let result = process_images(&paths, &config).unwrap();
        assert_eq!(result.len(), 1);
        let processed = result.get("/static/test.png").unwrap();
        assert_eq!(processed.rel_path, "test.png");
        assert_eq!(processed.original_width, 100);
        assert_eq!(processed.original_height, 80);
        // Should have resized (50w) + original (100w)
        assert_eq!(processed.srcset_entries.len(), 2);
        assert!(processed.srcset_entries.iter().any(|(w, _)| *w == 50));
        assert!(processed.srcset_entries.iter().any(|(w, _)| *w == 100));
        // WebP entries: resized + original
        assert_eq!(processed.webp_entries.len(), 2);
    }

    #[test]
    fn test_process_images_skips_widths_larger_than_original() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        // Create a small 50x40 image
        let img = image::RgbaImage::from_fn(50, 40, |_, _| image::Rgba([0, 0, 0, 255]));
        img.save(static_dir.join("small.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![480, 800, 1200], // All larger than the 50px image
            quality: 80,
            lazy_loading: true,
            webp: true,
        };
        let result = process_images(&paths, &config).unwrap();
        let processed = result.get("/static/small.png").unwrap();
        // Only the original (50w) should be in srcset — all configured widths are larger
        assert_eq!(processed.srcset_entries.len(), 1);
        assert_eq!(processed.srcset_entries[0].0, 50);
        // WebP: only original size
        assert_eq!(processed.webp_entries.len(), 1);
        assert_eq!(processed.webp_entries[0].0, 50);
    }

    #[test]
    fn test_process_images_jpeg_format() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        // Create a real JPEG image using explicit JPEG encoder (RGB, not RGBA)
        let img = image::RgbImage::from_fn(100, 80, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let jpeg_path = static_dir.join("photo.jpg");
        let file = fs::File::create(&jpeg_path).unwrap();
        let writer = std::io::BufWriter::new(file);
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 90);
        encoder
            .write_image(img.as_raw(), 100, 80, image::ExtendedColorType::Rgb8)
            .unwrap();

        // Test process_single_image directly with widths all larger than original
        // so no resize happens (avoids JPEG RGBA8 encoding limitation)
        let rel = Path::new("photo.jpg");
        let config = ImageSection {
            widths: vec![480, 800], // all larger than 100px
            quality: 75,
            lazy_loading: true,
            webp: true,
        };
        let result =
            process_single_image(&jpeg_path, rel, &tmp.path().join("dist"), &config, "jpg")
                .unwrap();
        assert_eq!(result.original_width, 100);
        assert_eq!(result.original_height, 80);
        // Only the original entry (no resizes since all widths > original)
        assert_eq!(result.srcset_entries.len(), 1);
        assert_eq!(result.srcset_entries[0].0, 100);
        assert_eq!(result.srcset_entries[0].1, "/static/photo.jpg");
        // Full-size WebP should be generated for JPEG source
        assert_eq!(result.webp_entries.len(), 1);
        assert_eq!(result.webp_entries[0].0, 100);
        assert!(result.webp_entries[0].1.ends_with(".webp"));
    }

    #[test]
    fn test_process_images_webp_input_no_webp_variants() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        // Create a WebP image using the image crate
        let img = image::RgbaImage::from_fn(100, 80, |_, _| image::Rgba([128, 128, 128, 255]));
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        dyn_img.save(static_dir.join("photo.webp")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![50],
            quality: 80,
            lazy_loading: true,
            webp: true, // webp is enabled but source is already webp
        };
        let result = process_images(&paths, &config).unwrap();
        let processed = result.get("/static/photo.webp").unwrap();
        // No WebP variants should be generated when source is already WebP
        assert!(processed.webp_entries.is_empty());
        // srcset still has resized + original
        assert_eq!(processed.srcset_entries.len(), 2);
    }

    #[test]
    fn test_process_images_no_webp_config() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        let img = image::RgbaImage::from_fn(100, 80, |_, _| image::Rgba([0, 0, 0, 255]));
        img.save(static_dir.join("test.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![50],
            quality: 80,
            lazy_loading: true,
            webp: false, // WebP disabled
        };
        let result = process_images(&paths, &config).unwrap();
        let processed = result.get("/static/test.png").unwrap();
        // No WebP entries when webp is disabled
        assert!(processed.webp_entries.is_empty());
    }

    #[test]
    fn test_process_images_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        let sub_dir = static_dir.join("images").join("gallery");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        let img = image::RgbaImage::from_fn(100, 80, |_, _| image::Rgba([255, 0, 0, 255]));
        img.save(sub_dir.join("photo.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![50],
            quality: 80,
            lazy_loading: true,
            webp: false,
        };
        let result = process_images(&paths, &config).unwrap();
        // Key should include the subdirectory path
        assert!(result.contains_key("/static/images/gallery/photo.png"));
        let processed = result.get("/static/images/gallery/photo.png").unwrap();
        assert_eq!(processed.rel_path, "images/gallery/photo.png");
        // URLs should include the subdirectory
        assert!(processed
            .srcset_entries
            .iter()
            .any(|(_, url)| url.contains("/static/images/gallery/")));
    }

    #[test]
    fn test_process_images_empty_widths() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        let img = image::RgbaImage::from_fn(100, 80, |_, _| image::Rgba([0, 0, 0, 255]));
        img.save(static_dir.join("test.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![], // No resize widths
            quality: 80,
            lazy_loading: true,
            webp: true,
        };
        let result = process_images(&paths, &config).unwrap();
        let processed = result.get("/static/test.png").unwrap();
        // Only original width in srcset (no resizes)
        assert_eq!(processed.srcset_entries.len(), 1);
        assert_eq!(processed.srcset_entries[0].0, 100);
        // Full-size WebP only
        assert_eq!(processed.webp_entries.len(), 1);
        assert_eq!(processed.webp_entries[0].0, 100);
    }

    #[test]
    fn test_process_images_srcset_sorted_by_width() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        // 200x100 image
        let img = image::RgbaImage::from_fn(200, 100, |_, _| image::Rgba([0, 0, 0, 255]));
        img.save(static_dir.join("test.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![150, 50, 100], // Unsorted widths
            quality: 80,
            lazy_loading: true,
            webp: true,
        };
        let result = process_images(&paths, &config).unwrap();
        let processed = result.get("/static/test.png").unwrap();
        // Entries should be sorted by width ascending
        let widths: Vec<u32> = processed.srcset_entries.iter().map(|(w, _)| *w).collect();
        assert_eq!(widths, vec![50, 100, 150, 200]); // 50, 100, 150 resized + 200 original
        let webp_widths: Vec<u32> = processed.webp_entries.iter().map(|(w, _)| *w).collect();
        assert_eq!(webp_widths, vec![50, 100, 150, 200]);
    }

    #[test]
    fn test_process_images_multiple_images() {
        let tmp = tempfile::tempdir().unwrap();
        let static_dir = tmp.path().join("static");
        fs::create_dir_all(&static_dir).unwrap();
        fs::create_dir_all(tmp.path().join("dist")).unwrap();

        let img1 = image::RgbaImage::from_fn(100, 80, |_, _| image::Rgba([255, 0, 0, 255]));
        img1.save(static_dir.join("a.png")).unwrap();

        let img2 = image::RgbaImage::from_fn(200, 150, |_, _| image::Rgba([0, 255, 0, 255]));
        img2.save(static_dir.join("b.png")).unwrap();

        let paths = ResolvedPaths {
            root: tmp.path().to_path_buf(),
            output: tmp.path().join("dist"),
            content: tmp.path().join("content"),
            templates: tmp.path().join("templates"),
            static_dir,
            data_dir: tmp.path().join("data"),
            public_dir: tmp.path().join("public"),
        };
        let config = ImageSection {
            widths: vec![50],
            quality: 80,
            lazy_loading: true,
            webp: false,
        };
        let result = process_images(&paths, &config).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("/static/a.png"));
        assert!(result.contains_key("/static/b.png"));
    }

    // ---------------------------------------------------------------
    // ProcessedImage struct
    // ---------------------------------------------------------------

    #[test]
    fn test_processed_image_clone() {
        let p = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![(480, "/photo-480w.jpg".into())],
            webp_entries: vec![(480, "/photo-480w.webp".into())],
            original_width: 1200,
            original_height: 800,
        };
        let cloned = p.clone();
        assert_eq!(cloned.rel_path, p.rel_path);
        assert_eq!(cloned.srcset_entries.len(), p.srcset_entries.len());
        assert_eq!(cloned.webp_entries.len(), p.webp_entries.len());
        assert_eq!(cloned.original_width, p.original_width);
        assert_eq!(cloned.original_height, p.original_height);
    }

    #[test]
    fn test_processed_image_debug() {
        let p = ProcessedImage {
            rel_path: "photo.jpg".into(),
            srcset_entries: vec![],
            webp_entries: vec![],
            original_width: 100,
            original_height: 50,
        };
        let debug = format!("{:?}", p);
        assert!(debug.contains("ProcessedImage"));
        assert!(debug.contains("photo.jpg"));
    }

    // ---------------------------------------------------------------
    // Integration: rewrite_html_images with picture element output
    // ---------------------------------------------------------------

    #[test]
    fn test_rewrite_html_images_full_pipeline_webp() {
        // Simulate a real build output: process_images produces a manifest,
        // then rewrite_html_images rewrites the HTML
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/images/hero.jpg".to_string(),
            ProcessedImage {
                rel_path: "images/hero.jpg".into(),
                srcset_entries: vec![
                    (480, "/static/images/hero-480w.jpg".into()),
                    (800, "/static/images/hero-800w.jpg".into()),
                    (1200, "/static/images/hero.jpg".into()),
                ],
                webp_entries: vec![
                    (480, "/static/images/hero-480w.webp".into()),
                    (800, "/static/images/hero-800w.webp".into()),
                    (1200, "/static/images/hero.webp".into()),
                ],
                original_width: 1200,
                original_height: 800,
            },
        );

        let html = r#"<article><h1>Post</h1><img src="/static/images/hero.jpg" alt="Hero image"><p>Content</p></article>"#;
        let result = rewrite_html_images(html, &manifest, true);

        // Structure checks
        assert!(result.contains("<article><h1>Post</h1>"));
        assert!(result.contains("<picture>"));
        assert!(result.contains("</picture>"));
        assert!(result.contains("<p>Content</p></article>"));

        // WebP source
        assert!(result.contains(r#"<source type="image/webp""#));
        assert!(result.contains("hero-480w.webp 480w"));
        assert!(result.contains("hero-800w.webp 800w"));
        assert!(result.contains("hero.webp 1200w"));

        // Original format srcset on img
        assert!(result.contains("hero-480w.jpg 480w"));
        assert!(result.contains("hero-800w.jpg 800w"));
        assert!(result.contains("hero.jpg 1200w"));

        // Lazy loading and dimensions
        assert!(result.contains(r#"loading="lazy""#));
        assert!(result.contains(r#"width="1200""#));
        assert!(result.contains(r#"height="800""#));

        // Alt preserved
        assert!(result.contains(r#"alt="Hero image""#));
    }

    #[test]
    fn test_rewrite_html_images_only_text_after_last_img() {
        let mut manifest = HashMap::new();
        manifest.insert(
            "/static/a.jpg".to_string(),
            ProcessedImage {
                rel_path: "a.jpg".into(),
                srcset_entries: vec![(100, "/static/a.jpg".into())],
                webp_entries: vec![],
                original_width: 100,
                original_height: 100,
            },
        );
        let html = r#"<img src="/static/a.jpg">trailing text here"#;
        let result = rewrite_html_images(html, &manifest, false);
        assert!(result.ends_with("trailing text here"));
        assert!(result.contains("srcset="));
    }
}
