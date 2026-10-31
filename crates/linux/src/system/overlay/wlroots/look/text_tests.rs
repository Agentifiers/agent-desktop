use agent_desktop_core::Rect;

use super::super::font::locate;
use super::super::raster::Surface;
use super::{draw_label, fitted};

fn opaque(width: i32, height: i32) -> Surface {
    let mut surface = Surface::try_transparent(width, height).expect("surface");
    surface
        .pixels
        .iter_mut()
        .for_each(|pixel| *pixel = 0xFFFF_FFFF);
    surface
}

#[test]
fn a_label_inks_the_card_only_inside_its_rectangle() {
    let Some(font) = locate() else {
        return;
    };
    let mut surface = opaque(240, 40);
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        width: 200.0,
        height: 18.0,
    };
    draw_label(
        &mut surface,
        &rect,
        "Clicking eight",
        [0.0, 0.0, 0.0],
        (13.0, &font),
    );
    let inked = |inside: bool| {
        (0..40)
            .flat_map(|y| (0..240).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let within = (8..215).contains(&x) && (6..32).contains(&y);
                within == inside && surface.pixel_at(x, y) != Some(0xFFFF_FFFF)
            })
            .count()
    };
    assert!(inked(true) > 80, "{}", inked(true));
    assert_eq!(inked(false), 0);
    assert!(surface.pixels.iter().all(|pixel| pixel >> 24 == 0xFF));
}

#[test]
fn a_label_too_wide_for_the_card_is_cut_with_an_ellipsis() {
    let Some(font) = locate() else {
        return;
    };
    let long = "Opening the settings window to change the default printer";
    let shown = fitted(&font, long, 13.0, 120.0);
    assert!(shown.ends_with('…') && shown.len() < long.len(), "{shown}");
    assert_eq!(fitted(&font, "Save", 13.0, 120.0), "Save");
    assert_eq!(fitted(&font, "Save", 13.0, 0.0), "");
}
