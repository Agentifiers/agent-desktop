use super::*;

#[test]
fn argb_words_are_stored_blue_first_and_keep_alpha() {
    assert_eq!(PixelLayout::Argb8888.rgba([1, 2, 3, 4]), [3, 2, 1, 4]);
    assert_eq!(PixelLayout::Xrgb8888.rgba([1, 2, 3, 4]), [3, 2, 1, 255]);
    assert_eq!(PixelLayout::Abgr8888.rgba([1, 2, 3, 4]), [1, 2, 3, 4]);
    assert_eq!(PixelLayout::Xbgr8888.rgba([1, 2, 3, 0]), [1, 2, 3, 255]);
}

#[test]
fn every_layout_round_trips_its_shm_code() {
    for layout in PixelLayout::PREFERRED {
        assert_eq!(PixelLayout::from_shm(layout.shm_code()), Some(layout));
    }
    assert_eq!(PixelLayout::from_shm(0x3432_5258 + 7), None);
}

#[test]
fn padded_rows_are_cut_to_the_width() {
    let rows = [10, 20, 30, 255, 0, 0, 0, 0, 40, 50, 60, 255, 0, 0, 0, 0];
    let image =
        CapturedImage::from_rows(&rows, 1, 2, 8, PixelLayout::Abgr8888, false).expect("decodes");
    assert_eq!(image.rgba, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn an_inverted_buffer_is_flipped_to_top_first() {
    let rows = [1, 1, 1, 255, 2, 2, 2, 255];
    let image =
        CapturedImage::from_rows(&rows, 1, 2, 4, PixelLayout::Abgr8888, true).expect("decodes");
    assert_eq!(image.to_rgb(), vec![2, 2, 2, 1, 1, 1]);
}

#[test]
fn a_short_buffer_is_refused() {
    let error = CapturedImage::from_rows(&[0; 4], 1, 2, 4, PixelLayout::Abgr8888, false)
        .expect_err("short");
    assert_eq!(error.code, ErrorCode::ActionFailed);
}
