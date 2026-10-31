/// How a `wl_shm` pixel's four bytes are ordered in memory. The format names
/// are read from a little-endian 32-bit word, so `ARGB8888` is stored as
/// blue, green, red, alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PixelLayout {
    Argb8888,
    Xrgb8888,
    Abgr8888,
    Xbgr8888,
}

const ARGB8888: u32 = 0;
const XRGB8888: u32 = 1;
const ABGR8888: u32 = 0x3432_4241;
const XBGR8888: u32 = 0x3432_4258;

impl PixelLayout {
    pub(crate) fn from_shm(format: u32) -> Option<Self> {
        match format {
            ARGB8888 => Some(Self::Argb8888),
            XRGB8888 => Some(Self::Xrgb8888),
            ABGR8888 => Some(Self::Abgr8888),
            XBGR8888 => Some(Self::Xbgr8888),
            _ => None,
        }
    }

    /// The `wl_shm` format code, the preference order a capture picks from.
    pub(crate) fn shm_code(self) -> u32 {
        match self {
            Self::Argb8888 => ARGB8888,
            Self::Xrgb8888 => XRGB8888,
            Self::Abgr8888 => ABGR8888,
            Self::Xbgr8888 => XBGR8888,
        }
    }

    pub(crate) const PREFERRED: [Self; 4] = [
        Self::Argb8888,
        Self::Abgr8888,
        Self::Xrgb8888,
        Self::Xbgr8888,
    ];

    /// One pixel's red, green, blue and alpha. A format without alpha is
    /// opaque.
    pub(crate) fn rgba(self, bytes: [u8; 4]) -> [u8; 4] {
        let [b0, b1, b2, b3] = bytes;
        match self {
            Self::Argb8888 => [b2, b1, b0, b3],
            Self::Xrgb8888 => [b2, b1, b0, 0xff],
            Self::Abgr8888 => [b0, b1, b2, b3],
            Self::Xbgr8888 => [b0, b1, b2, 0xff],
        }
    }
}
