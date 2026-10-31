use agent_desktop_core::CursorOverlayStyle;

use super::arrow;

const WHITE: u32 = 0xffff_ffff;
const RIM: u32 = 0xff11_1318;

#[test]
fn the_default_arrow_has_the_x11_size_tip_fill_and_rim() {
    let raster = arrow(&CursorOverlayStyle::default());
    assert_eq!((raster.width, raster.height), (24, 32));
    assert_eq!(raster.at(0, 0), Some(RIM));
    assert_eq!(raster.at(0, 12), Some(RIM));
    assert_eq!(raster.at(3, 12), Some(WHITE));
    assert_eq!(raster.at(12, 22), Some(WHITE));
}

#[test]
fn everything_outside_the_outline_is_transparent() {
    let raster = arrow(&CursorOverlayStyle::default());
    for (x, y) in [(20, 2), (23, 31), (2, 30), (23, 0)] {
        assert_eq!(raster.at(x, y), Some(0), "({x}, {y})");
    }
    let opaque = raster.pixels.iter().filter(|pixel| **pixel != 0).count();
    assert!(opaque > 150 && opaque < 24 * 32 / 2, "{opaque}");
}

#[test]
fn size_and_colours_follow_the_style() {
    let mut style = CursorOverlayStyle::default();
    style.set_fill("#FF0000".into());
    style.set_rim("#0000FF".into());
    style.set_size(2.0);
    let raster = arrow(&style);
    assert_eq!((raster.width, raster.height), (48, 64));
    assert_eq!(raster.at(0, 0), Some(0xff00_00ff));
    assert_eq!(raster.at(6, 24), Some(0xffff_0000));
}

#[test]
fn shm_bytes_are_little_endian_argb() {
    let raster = arrow(&CursorOverlayStyle::default());
    let bytes = raster.to_shm_bytes();
    assert_eq!(bytes.len(), 24 * 32 * 4);
    assert_eq!(&bytes[..4], &[0x18, 0x13, 0x11, 0xff]);
}
