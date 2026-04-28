use command::blocking::Command;

use super::util::main_display_scale_factor;
use crate::ScreenshotParams;

/// Captures a screenshot of the main display using the built-in macOS
/// `screencapture` CLI.
pub fn take(params: ScreenshotParams) -> Result<crate::Screenshot, String> {
    let output_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temporary directory for screenshot: {e}"))?;
    let output_path = output_dir.path().join("screenshot.png");

    let mut cmd = Command::new("/usr/sbin/screencapture");
    cmd.args([
        "-x",    // Do not play sounds.
        "-tpng", // Capture to PNG format.
        "-m",    // Only capture the main display (not all displays).
    ]);

    if let Some(region) = params.region {
        region.validate()?;
        // -R x,y,w,h captures a specific rectangle in point coordinates.
        // Convert from physical pixel coordinates to point coordinates.
        let scale = main_display_scale_factor();
        let x = (region.top_left.x() as f64 / scale) as i32;
        let y = (region.top_left.y() as f64 / scale) as i32;
        let w = ((region.bottom_right.x() - region.top_left.x()) as f64 / scale) as i32;
        let h = ((region.bottom_right.y() - region.top_left.y()) as f64 / scale) as i32;
        cmd.arg("-R").arg(format!("{x},{y},{w},{h}"));
    }

    let output = cmd
        .arg(&output_path)
        .output()
        .map_err(|e| format!("Failed to run screencapture: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            format!("exit code {}", output.status)
        } else {
            format!("exit code {}: {}", output.status, stderr.trim())
        };
        return Err(format!("screencapture failed with {detail}"));
    }

    crate::screenshot_utils::load_and_process_screenshot(&output_path, params)
}
