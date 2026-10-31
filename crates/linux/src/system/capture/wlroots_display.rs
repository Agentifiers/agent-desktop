//! Display capture on a wlroots compositor through `zwlr_screencopy_v1`: no
//! portal, no consent dialog, and no cursor in the image.

use agent_desktop_core::{AdapterError, Deadline, ImageBuffer, ScreenshotTarget};

use super::display::{invalid, select};
use super::wayland_capture::CopyOutput;
use crate::system::capture::capture_env::CaptureEnv;
use crate::system::wlroots::captured_image::CapturedImage;
use crate::system::wlroots::output::Output;
use crate::system::wlroots::screencopy::capture_outputs;

fn changed(count: usize) -> AdapterError {
    invalid(
        "The display changed while it was being captured".to_owned(),
        count,
    )
}

/// Copies the output called `name` with the compositor's screencopy.
pub(crate) fn copy_output(
    name: &str,
    deadline: Deadline,
) -> Result<(Output, CapturedImage), AdapterError> {
    let mut copies = capture_outputs(|output| output.name == name, deadline)?;
    match (copies.pop(), copies.is_empty()) {
        (Some(copy), true) => Ok(copy),
        _ => Err(changed(0)),
    }
}

/// Captures the display `target` names, proving before and after the copy
/// that it is still the display the target named. The image holds the
/// output's own pixels, so its scale is the display's.
pub(crate) fn capture(
    env: &CaptureEnv,
    target: &ScreenshotTarget,
    deadline: Deadline,
    copy: CopyOutput<'_>,
) -> Result<ImageBuffer, AdapterError> {
    deadline.ensure_remaining()?;
    let displays = env.lookup.displays(deadline)?;
    let (index, display) = select(&displays, target)?;
    let (output, image) = copy(&display.id, deadline)?;
    if output.rect() != display.bounds {
        return Err(changed(displays.len()));
    }
    deadline.ensure_remaining()?;
    let after = env.lookup.displays(deadline)?;
    if after.get(index) != Some(&display) {
        return Err(changed(after.len()));
    }
    (env.encode)(&image.to_rgb(), image.width, image.height, display.scale)
}

#[cfg(test)]
#[path = "wlroots_display_tests.rs"]
mod tests;
