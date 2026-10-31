/// The states `zwlr_foreign_toplevel_handle_v1` reports for one toplevel,
/// as of its latest `done`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ToplevelFlags {
    pub(crate) activated: bool,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) fullscreen: bool,
}

const MAXIMIZED: u32 = 0;
const MINIMIZED: u32 = 1;
const ACTIVATED: u32 = 2;
const FULLSCREEN: u32 = 3;

impl ToplevelFlags {
    /// The flags of a `state` event, whose array holds one native-endian
    /// `u32` per state the toplevel is in.
    pub(crate) fn from_state_array(bytes: &[u8]) -> Self {
        let mut flags = Self::default();
        for chunk in bytes.chunks_exact(4) {
            let mut word = [0_u8; 4];
            word.copy_from_slice(chunk);
            match u32::from_ne_bytes(word) {
                MAXIMIZED => flags.maximized = true,
                MINIMIZED => flags.minimized = true,
                ACTIVATED => flags.activated = true,
                FULLSCREEN => flags.fullscreen = true,
                _ => {}
            }
        }
        flags
    }
}

#[cfg(test)]
#[path = "toplevel_flags_tests.rs"]
mod tests;
