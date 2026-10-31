use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) use super::pixel_layout::PixelLayout;

/// Pixels copied out of the compositor, four bytes per pixel in red, green,
/// blue, alpha order, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedImage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

fn malformed(reason: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The captured pixels are unusable: {reason}"),
    )
}

impl CapturedImage {
    /// Decodes `rows` of `stride` bytes laid out as `layout`; a `y_inverted`
    /// buffer is stored bottom row first.
    pub(crate) fn from_rows(
        rows: &[u8],
        width: u32,
        height: u32,
        stride: usize,
        layout: PixelLayout,
        y_inverted: bool,
    ) -> Result<Self, AdapterError> {
        let row_bytes = width as usize * 4;
        if stride < row_bytes || rows.len() < stride * height as usize {
            return Err(malformed("the buffer is shorter than its size says"));
        }
        let mut rgba = Vec::with_capacity(row_bytes * height as usize);
        for row in 0..height as usize {
            let source = if y_inverted {
                height as usize - 1 - row
            } else {
                row
            };
            let line = &rows[source * stride..source * stride + row_bytes];
            for pixel in line.chunks_exact(4) {
                rgba.extend_from_slice(&layout.rgba([pixel[0], pixel[1], pixel[2], pixel[3]]));
            }
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// The pixel at `(x, y)`, or `None` outside the image.
    pub(crate) fn pixel(&self, x: i64, y: i64) -> Option<[u8; 4]> {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return None;
        }
        let at = (y as usize * self.width as usize + x as usize) * 4;
        let bytes = self.rgba.get(at..at + 4)?;
        Some([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// The image without its alpha channel, three bytes per pixel.
    pub(crate) fn to_rgb(&self) -> Vec<u8> {
        self.rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect()
    }
}

#[cfg(test)]
#[path = "captured_image_tests.rs"]
mod tests;
