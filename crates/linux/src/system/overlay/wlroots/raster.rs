//! A small premultiplied ARGB image laid out the way a `wl_shm` ARGB8888
//! buffer holds it: one little-endian 32-bit word per pixel, row after row.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Raster {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u32>,
}

impl Raster {
    pub(crate) fn transparent(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize],
        }
    }

    #[cfg(test)]
    pub(crate) fn at(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width {
            return None;
        }
        self.pixels
            .get(y as usize * self.width as usize + x as usize)
            .copied()
    }

    pub(crate) fn set(&mut self, x: i32, y: i32, pixel: u32) {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return;
        };
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y as usize * self.width as usize + x as usize;
        if let Some(slot) = self.pixels.get_mut(index) {
            *slot = pixel;
        }
    }

    /// The bytes a `wl_shm` ARGB8888 buffer of this size holds.
    pub(crate) fn to_shm_bytes(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .collect()
    }
}
