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
}
