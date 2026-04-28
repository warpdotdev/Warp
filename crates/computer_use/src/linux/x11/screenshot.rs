//! Screenshot capture for X11.

use x11rb::protocol::xproto::{self, ConnectionExt as _, ImageFormat};
use x11rb::rust_connection::RustConnection;

use crate::{Screenshot, ScreenshotParams};

/// Takes a screenshot of the root window or a region of it.
pub fn take(
    conn: &RustConnection,
    screen: &xproto::Screen,
    root: xproto::Window,
    params: ScreenshotParams,
) -> Result<Screenshot, String> {
    // Determine the capture region.
    let (x, y, width, height) = if let Some(region) = params.region {
        region.validate()?;
        let x = region.top_left.x() as i16;
        let y = region.top_left.y() as i16;
        let width = (region.bottom_right.x() - region.top_left.x()) as u16;
        let height = (region.bottom_right.y() - region.top_left.y()) as u16;
        (x, y, width, height)
    } else {
        (0, 0, screen.width_in_pixels, screen.height_in_pixels)
    };

    // Get the image from the root window.
    // TODO: Consider compositing the cursor into the screenshot in the future.
    let image = conn
        .get_image(
            ImageFormat::Z_PIXMAP,
            root,
            x,
            y,
            width,
            height,
            !0, // plane_mask: all planes
        )
        .map_err(|e| format!("Failed to request screenshot: {e}"))?
        .reply()
        .map_err(|e| format!("Failed to get screenshot reply: {e}"))?;

    // Convert the X11 image data to an image::RgbImage.
    // X11 typically returns BGRA or BGR depending on depth.
    let depth = image.depth;
    let data = image.data;

    let rgb_data = convert_x11_image_to_rgb(&data, width as usize, height as usize, depth)?;

    let img = image::RgbImage::from_raw(width as u32, height as u32, rgb_data)
        .ok_or("Failed to create image from raw data")?;

    let img = image::DynamicImage::ImageRgb8(img);

    crate::screenshot_utils::process_screenshot(img, params)
}

/// Converts X11 image data (typically BGRA or BGR) to RGB.
fn convert_x11_image_to_rgb(
    data: &[u8],
    width: usize,
    height: usize,
    depth: u8,
) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity(width * height * 3);

    match depth {
        24 => {
            // 24-bit: BGR format, 3 bytes per pixel (but often padded to 4).
            // X11 often uses 32-bit alignment even for 24-bit depth.
            let bytes_per_pixel = if data.len() >= width * height * 4 {
                4
            } else {
                3
            };

            for y in 0..height {
                for x in 0..width {
                    let offset = (y * width + x) * bytes_per_pixel;
                    if offset + 2 < data.len() {
                        let b = data[offset];
                        let g = data[offset + 1];
                        let r = data[offset + 2];
                        rgb.push(r);
                        rgb.push(g);
                        rgb.push(b);
                    }
                }
            }
        }
        32 => {
            // 32-bit: BGRA format, 4 bytes per pixel.
            for y in 0..height {
                for x in 0..width {
                    let offset = (y * width + x) * 4;
                    if offset + 2 < data.len() {
                        let b = data[offset];
                        let g = data[offset + 1];
                        let r = data[offset + 2];
                        // Skip alpha at offset + 3.
                        rgb.push(r);
                        rgb.push(g);
                        rgb.push(b);
                    }
                }
            }
        }
        _ => {
            return Err(format!("Unsupported screen depth: {depth}"));
        }
    }

    Ok(rgb)
}
