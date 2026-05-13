#[allow(unused_imports)]
use crate::clipboard::{Clipboard, ClipboardContent};

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
use {arboard, image::ImageEncoder};

use itertools::Itertools;

/// Supported image file extensions for clipboard operations.
pub const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp"];

/// Preferred image MIME types for clipboard operations (in order of preference)
pub const CLIPBOARD_IMAGE_MIME_TYPES: &[&str] = &[
    "image/png",  // Preferred: lossless, good compression
    "image/jpeg", // Good fallback: widely supported
    "image/jpg",  // JPEG variant
    "image/gif",  // Animated images
    "image/webp", // Modern format but less compatible
];

/// Minimum bytes needed for image format detection.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
const MIN_IMAGE_HEADER_SIZE: usize = 8;

/// Check if a string has an image file extension.
pub fn has_image_extension(s: &str) -> bool {
    IMAGE_EXTENSIONS
        .iter()
        .any(|ext| s.to_lowercase().ends_with(ext))
}

/// Extract filename from a file path, handling file:// URLs and path separators.
fn extract_filename_from_path(path: &str) -> String {
    path.strip_prefix("file://")
        .unwrap_or(path)
        .split(['/', '\\'])
        .next_back()
        .unwrap_or(path)
        .to_string()
}

/// Extract filename from clipboard content (HTML or text).
/// Tries HTML first, then falls back to text content.
pub fn extract_filename_from_clipboard_content(
    html_content: &Option<String>,
    text_content: &str,
) -> Option<String> {
    html_content
        .as_ref()
        .and_then(|html| extract_filename_from_html(html))
        .or_else(|| extract_filename_from_text(text_content))
}

/// Extract filename from text content (file paths, URLs, etc.).
pub fn extract_filename_from_text(text: &str) -> Option<String> {
    // Early return for empty input
    if text.trim().is_empty() {
        return None;
    }

    // First, check if the entire text is a file path with an image extension
    let trimmed = text.trim();
    if trimmed.contains('.') && has_image_extension(trimmed) {
        return Some(extract_filename_from_path(trimmed));
    }

    // Look for file paths in the text
    for line in text.lines() {
        let line = line.trim();
        if line.contains('.') && has_image_extension(line) {
            return Some(extract_filename_from_path(line));
        }
    }

    None
}

/// Extract filename from HTML content.
pub fn extract_filename_from_html(html: &str) -> Option<String> {
    // Early return for empty HTML
    if html.trim().is_empty() {
        return None;
    }

    // First try to extract from HTML structure, then fall back to text extraction
    if let Some(filename) = extract_filename_from_html_tags(html) {
        return Some(filename);
    }

    // Fall back to treating HTML as plain text for file paths
    extract_filename_from_text(html)
}

/// Extract filename from HTML tags and attributes.
fn extract_filename_from_html_tags(html: &str) -> Option<String> {
    // Helper function to extract quoted attribute value
    let extract_quoted_value = |html: &str, attr_pattern: &str| -> Option<String> {
        html.find(attr_pattern)
            .and_then(|start| {
                let content_start = start + attr_pattern.len();
                html[content_start..].split('"').next()
            })
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    // 1. Check src attribute in img tag (most common case)
    if let Some(src_content) = extract_quoted_value(html, "src=\"") {
        let filename = extract_filename_from_path(&src_content);
        if filename.contains('.') && has_image_extension(&filename) {
            return Some(filename);
        }
    }

    // 2. Check title attribute
    if let Some(title_content) = extract_quoted_value(html, "title=\"") {
        if title_content.contains('.') && has_image_extension(&title_content) {
            return Some(title_content);
        }
    }

    // 3. Check alt attribute
    if let Some(alt_content) = extract_quoted_value(html, "alt=\"") {
        if alt_content.contains('.') && has_image_extension(&alt_content) {
            return Some(alt_content);
        }
    }

    // 4. Look for any filename-like strings with image extensions in the entire HTML
    const TRIM_CHARS: &[char] = &['"', '\'', '<', '>', '(', ')', ',', ';'];

    for word in html.split_whitespace() {
        if word.contains('.') {
            let clean_word = word.trim_matches(TRIM_CHARS);
            if has_image_extension(clean_word) {
                let filename = extract_filename_from_path(clean_word);
                return Some(filename);
            }
        }
    }

    None
}

/// Best-effort conversion of HTML clipboard contents to plain text.
///
/// This is intentionally lightweight (no external HTML parser dependency). It strips tags,
/// decodes a small set of common entities, and collapses whitespace.
pub fn strip_html_to_plain_text(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    // Fast path: if there are no obvious tag/entity markers, treat as plain text.
    if !html.contains('<') && !html.contains('&') {
        return html.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    fn decode_entity(entity: &str) -> Option<char> {
        match entity {
            "nbsp" => Some(' '),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "#39" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        }
    }

    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity_buf = String::new();
    let mut last_was_space = false;

    for ch in html.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
                // Treat tags as word boundaries.
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            continue;
        }

        if in_entity {
            if ch == ';' {
                let decoded = decode_entity(entity_buf.as_str());
                if let Some(decoded) = decoded {
                    if decoded.is_whitespace() {
                        if !last_was_space {
                            out.push(' ');
                            last_was_space = true;
                        }
                    } else {
                        out.push(decoded);
                        last_was_space = false;
                    }
                } else {
                    // Unknown entity; keep it as-is (best effort).
                    if !last_was_space {
                        out.push(' ');
                    }
                    out.push('&');
                    out.push_str(entity_buf.as_str());
                    out.push(';');
                    out.push(' ');
                    last_was_space = true;
                }
                entity_buf.clear();
                in_entity = false;
                continue;
            }

            // Guard against extremely long/unterminated entities.
            if entity_buf.len() >= 24 {
                in_entity = false;
                entity_buf.clear();
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
                continue;
            }

            entity_buf.push(ch);
            continue;
        }

        match ch {
            '<' => {
                in_tag = true;
                // Ensure words on either side of tags don't get glued together.
                if !last_was_space && !out.is_empty() {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            '&' => {
                in_entity = true;
                entity_buf.clear();
            }
            ch if ch.is_whitespace() => {
                if !last_was_space {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            _ => {
                out.push(ch);
                last_was_space = false;
            }
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Process clipboard image data, preserving original format or converting to PNG.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
pub fn process_clipboard_image(
    arboard_image: &arboard::ImageData,
    filename: Option<String>,
) -> Option<crate::clipboard::ImageData> {
    let result =
        try_preserve_original_format(&arboard_image.bytes, filename.clone()).or_else(|| {
            convert_raw_bitmap_to_png(
                arboard_image.width,
                arboard_image.height,
                arboard_image.bytes.to_vec(),
                filename,
            )
        });

    if result.is_none() {
        log::warn!(
            "Failed to process clipboard image: format preservation and PNG conversion both failed"
        );
    }

    result
}

/// Read image data from clipboard, checking for images before expensive filename extraction.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
pub fn read_images_from_clipboard(
    clipboard: &mut arboard::Clipboard,
    html_content: &Option<String>,
    text_content: &str,
) -> Option<Vec<crate::clipboard::ImageData>> {
    // First, quickly check if there are any images in the clipboard
    // This is a fast operation that avoids filename extraction overhead
    let arboard_result = clipboard.get().image();

    match arboard_result {
        Ok(arboard_image) => {
            // Images found! Now extract filename from clipboard content
            let filename = extract_filename_from_clipboard_content(html_content, text_content);

            // Process the image with the extracted filename
            match process_clipboard_image(&arboard_image, filename) {
                Some(image_data) => Some(vec![image_data]),
                None => {
                    log::warn!("Failed to process clipboard image: format detection and conversion both failed");
                    // Fall through to Windows custom-format fallback below.
                    #[cfg(target_os = "windows")]
                    {
                        let filename =
                            extract_filename_from_clipboard_content(html_content, text_content);
                        if let Some(img) = try_read_image_via_custom_windows_formats(filename) {
                            return Some(vec![img]);
                        }
                    }
                    None
                }
            }
        }
        // arboard saw no CF_DIB / CF_BITMAP, or failed to convert it.
        // On Windows, many screenshot tools (Snipaste / ShareX / QQ / 微信)
        // only write the registered "PNG" format, or write a malformed CF_DIB
        // alongside a real PNG payload. Try those raw formats before giving up.
        Err(err) => {
            #[cfg(target_os = "windows")]
            {
                if !matches!(err, arboard::Error::ContentNotAvailable) {
                    log::debug!("arboard image() failed ({err:?}); trying custom Windows formats");
                }
                let filename = extract_filename_from_clipboard_content(html_content, text_content);
                if let Some(img) = try_read_image_via_custom_windows_formats(filename) {
                    return Some(vec![img]);
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                if !matches!(err, arboard::Error::ContentNotAvailable) {
                    log::warn!("Unable to read image from clipboard: {err:?}");
                }
            }
            None
        }
    }
}

/// Windows 专用兜底: 只枚举看起来像图片的剪贴板格式,并寻找
/// PNG/JPEG/GIF/WebP 等已知图片字节。
///
/// 这用于兼容不写 CF_DIB、只注册 "PNG" 或 "image/png" 之类格式名的截图工具。
/// 不要探测任意注册格式: clipboard-win 读取原始字节时必须调用
/// GlobalSize/GlobalLock,而一些外部剪贴板格式不能安全地当成 HGLOBAL 字节缓冲读取。
#[cfg(target_os = "windows")]
fn try_read_image_via_custom_windows_formats(
    filename: Option<String>,
) -> Option<crate::clipboard::ImageData> {
    use clipboard_win::{raw, Clipboard, EnumFormats};

    // RAII OpenClipboard / CloseClipboard. arboard has already released the
    // clipboard by the time we get here (its `get().image()` call returned).
    let _clip = match Clipboard::new_attempts(10) {
        Ok(c) => c,
        Err(err) => {
            log::warn!("Custom-format fallback: OpenClipboard failed: {err:?}");
            return None;
        }
    };

    // 先收集候选格式,避免为了诊断而 raw-read 外部应用放入的每一种自定义格式。
    let formats: Vec<(u32, String)> = EnumFormats::new()
        .filter_map(|fmt| {
            let name = raw::format_name_big(fmt).unwrap_or_else(|| format!("<unknown {fmt:#06x}>"));
            is_windows_image_clipboard_format_candidate(fmt, &name).then_some((fmt, name))
        })
        .collect();
    if formats.is_empty() {
        return None;
    }

    let names: Vec<String> = formats.iter().map(|(_, name)| name.clone()).collect();
    log::info!(
        "Custom-format fallback: clipboard has {} image candidate format(s): {:?}",
        formats.len(),
        names
    );

    for (fmt, name) in formats {
        let mut buf: Vec<u8> = Vec::new();
        match raw::get_vec(fmt, &mut buf) {
            Ok(_) if !buf.is_empty() => {
                // 1) Try magic-byte detection (PNG / JPEG / GIF / WebP)
                if let Some(img) = try_preserve_original_format(&buf, filename.clone()) {
                    log::info!(
                        "Custom-format fallback: matched format {name:?} ({} bytes, mime={})",
                        buf.len(),
                        img.mime_type
                    );
                    return Some(img);
                }

                // 2) DIB decode path. CF_DIB/CF_DIBV5 carry headerless BMP
                // bytes. Some screenshot tools (PixPin, etc.) write a DIB that
                // arboard fails to parse but the `image` crate's BmpDecoder
                // handles fine via `new_without_file_header`.
                if is_windows_dib_clipboard_format(fmt) || looks_like_dib(&buf) {
                    if let Some(img) = try_decode_dib_to_png(&buf, filename.clone()) {
                        log::info!(
                            "Custom-format fallback: decoded DIB from format {name:?} ({} bytes → {} bytes PNG)",
                            buf.len(),
                            img.data.len()
                        );
                        return Some(img);
                    }
                }
            }
            Ok(_) => continue,
            Err(err) => {
                log::debug!("Custom-format fallback: GetClipboardData({fmt:#06x}) failed: {err:?}");
                continue;
            }
        }
    }

    log::info!("Custom-format fallback: no format on clipboard contained recognizable image bytes");
    None
}

#[cfg(target_os = "windows")]
fn is_windows_dib_clipboard_format(format: u32) -> bool {
    const CF_DIB: u32 = 8;
    const CF_DIBV5: u32 = 17;

    format == CF_DIB || format == CF_DIBV5
}

#[cfg(target_os = "windows")]
fn is_windows_image_clipboard_format_candidate(format: u32, name: &str) -> bool {
    if is_windows_dib_clipboard_format(format) {
        return true;
    }

    // 只有格式名明确表示图片字节的注册格式才足够适合 raw-read。
    // 这样保留常见截图工具支持,同时文本粘贴时不会触碰浏览器/编辑器的任意私有格式。
    let normalized = name.to_ascii_lowercase();
    ["png", "jpeg", "jpg", "gif", "webp", "image/"]
        .iter()
        .any(|token| normalized.contains(token))
}

/// Detect whether a byte slice plausibly starts with a Windows DIB header.
/// The first 4 bytes of a DIB are `biSize` (header size, little-endian).
/// Valid header sizes: 12 (BITMAPCOREHEADER), 40 (BITMAPINFOHEADER),
/// 52/56/64 (Adobe variants), 108 (BITMAPV4HEADER), 124 (BITMAPV5HEADER).
#[cfg(target_os = "windows")]
fn looks_like_dib(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    matches!(size, 12 | 40 | 52 | 56 | 64 | 108 | 124)
}

/// Decode a headerless DIB (no BITMAPFILEHEADER) into PNG bytes via the
/// `image` crate's BMP decoder. Returns `None` on decode failure.
#[cfg(target_os = "windows")]
fn try_decode_dib_to_png(
    bytes: &[u8],
    filename: Option<String>,
) -> Option<crate::clipboard::ImageData> {
    use image::codecs::bmp::BmpDecoder;
    use image::{DynamicImage, ImageFormat};
    use std::io::Cursor;

    let decoder = match BmpDecoder::new_without_file_header(Cursor::new(bytes)) {
        Ok(d) => d,
        Err(err) => {
            log::debug!("DIB decode: BmpDecoder init failed: {err:?}");
            return None;
        }
    };

    let dyn_img = match DynamicImage::from_decoder(decoder) {
        Ok(img) => img,
        Err(err) => {
            log::debug!("DIB decode: DynamicImage::from_decoder failed: {err:?}");
            return None;
        }
    };

    let mut png_bytes: Vec<u8> = Vec::new();
    if let Err(err) = dyn_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
        log::debug!("DIB decode: PNG encode failed: {err:?}");
        return None;
    }

    Some(crate::clipboard::ImageData {
        data: png_bytes,
        mime_type: "image/png".to_string(),
        filename,
    })
}

/// Try to preserve original image format using infer crate for detection.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
pub fn try_preserve_original_format(
    bytes: &[u8],
    filename: Option<String>,
) -> Option<crate::clipboard::ImageData> {
    if bytes.len() < MIN_IMAGE_HEADER_SIZE {
        return None;
    }

    // Use infer crate to detect the image format
    if let Some(kind) = infer::get(bytes) {
        // Check if it's a supported image format
        match kind.mime_type() {
            "image/png" | "image/jpeg" | "image/gif" | "image/webp" => {
                return Some(crate::clipboard::ImageData {
                    data: bytes.to_vec(),
                    mime_type: kind.mime_type().to_string(),
                    filename,
                });
            }
            _ => {}
        }
    }
    None
}

/// Converts RGBA bitmap data to PNG format, returns None on invalid dimensions/encoding.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "windows"))]
pub fn convert_raw_bitmap_to_png(
    width: usize,
    height: usize,
    bytes: Vec<u8>,
    filename: Option<String>,
) -> Option<crate::clipboard::ImageData> {
    // Validate dimensions before processing
    let width_u32 = match width.try_into() {
        Ok(w) => w,
        Err(e) => {
            log::warn!("Invalid width for PNG conversion: {width} - {e}");
            return None;
        }
    };

    let height_u32 = match height.try_into() {
        Ok(h) => h,
        Err(e) => {
            log::warn!("Invalid height for PNG conversion: {height} - {e}");
            return None;
        }
    };

    // Create RGBA image buffer from raw data
    // Note: arboard should already provide data in RGBA format
    let img_buffer =
        image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width_u32, height_u32, bytes)?;

    // Encode as PNG with optimized settings for speed
    let mut png_data = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_data);

    // Use fast compression settings to reduce encoding time
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut cursor,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    );

    let encode_result = encoder.write_image(
        &img_buffer,
        width_u32,
        height_u32,
        image::ColorType::Rgba8.into(),
    );

    match encode_result {
        Ok(_) => Some(crate::clipboard::ImageData {
            data: png_data,
            mime_type: "image/png".to_string(),
            filename,
        }),
        Err(err) => {
            log::warn!("PNG encoding failed: {err:?}");
            None
        }
    }
}

pub fn get_image_filepaths_from_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| has_image_extension(path))
        .cloned()
        .collect()
}

/// Create escaped file paths text string for insertion into terminal.
pub fn escaped_paths_str(
    paths: &[String],
    shell_family: Option<warp_util::path::ShellFamily>,
) -> String {
    // Handle regular file paths as text
    #[allow(unused_mut)]
    let mut input = paths
        .iter()
        .map(|path| match shell_family {
            Some(shell_family) => shell_family.escape(path.as_ref()),
            None => std::borrow::Cow::Borrowed(path.as_ref()),
        })
        .join(" ");

    // Append a space in case of back-to-back drag-drops.
    input.push(' ');

    input
}

#[cfg(test)]
#[path = "clipboard_utils_tests.rs"]
mod tests;
