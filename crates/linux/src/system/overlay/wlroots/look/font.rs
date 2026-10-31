//! The sans-serif face the label card is written in, read from the system at
//! run time. Nothing is bundled: a system without a readable sans face draws
//! no card, so no caption is ever shown as an empty box.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const KNOWN: [&str; 8] = [
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
];
const MAX_FONT_BYTES: u64 = 32 * 1024 * 1024;

fn matched() -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}", "sans-serif"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = String::from_utf8(output.stdout).ok()?;
    (output.status.success() && !path.is_empty()).then(|| PathBuf::from(path))
}

fn load(path: &Path) -> Option<fontdue::Font> {
    if std::fs::metadata(path).ok()?.len() > MAX_FONT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

/// The face fontconfig names for `sans-serif`, else the first well-known
/// sans face that loads, else none.
pub(crate) fn locate() -> Option<fontdue::Font> {
    matched()
        .into_iter()
        .chain(KNOWN.iter().map(PathBuf::from))
        .find_map(|path| load(&path))
}
