use super::super::compose;
use super::{frame, pill, screen};
use agent_desktop_core::{CursorOverlayStyle, Point};

/// The label's rectangle is where GDI is allowed to write, and it has to sit
/// inside the surface or the text lands outside the window entirely.
#[test]
fn a_label_yields_a_text_rectangle_inside_the_surface() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some(pill(80.0))));

    let text = composed.text_rect.expect("a label yields a text rectangle");
    assert!(text.x >= 0.0 && text.y >= 0.0);
    assert!(text.x + text.width <= f64::from(composed.surface.width));
    assert!(text.y + text.height <= f64::from(composed.surface.height));
    assert!(text.width > 0.0 && text.height > 0.0);
}

/// The pill fits its text: a longer label yields a wider card, and the text
/// sits centred in it rather than against its left end.
#[test]
fn the_card_fits_its_text_and_centres_it() {
    let style = CursorOverlayStyle::default();

    let short = compose(&frame(&style, Some(pill(30.0))));
    let long = compose(&frame(&style, Some(pill(200.0))));

    let card = |composed: &super::super::Composed| composed.card_rect.expect("a card");
    assert!(card(&long).width > card(&short).width + 150.0);
    for composed in [&short, &long] {
        let text = composed.text_rect.expect("a text rectangle");
        let card = card(composed);
        let centre = |x: f64, width: f64| x + width / 2.0;
        assert!((centre(text.x, text.width) - centre(card.x, card.width)).abs() < 0.01);
    }
}

/// The bubble body must be opaque under the text, because that is what makes
/// forcing the text rectangle's alpha correct rather than a workaround.
#[test]
fn the_bubble_body_is_opaque_beneath_where_the_text_will_go() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some(pill(80.0))));
    let text = composed.text_rect.expect("a text rectangle");

    let alpha = composed.surface.alpha_at(
        (text.x + text.width / 2.0) as i32,
        (text.y + text.height / 2.0) as i32,
    );
    assert_eq!(
        alpha, 255,
        "GDI writes RGB without alpha, so it may only draw where the body is already opaque"
    );
    for (x, y) in [
        (text.x, text.y),
        (text.x + text.width - 1.0, text.y),
        (text.x, text.y + text.height - 1.0),
        (text.x + text.width - 1.0, text.y + text.height - 1.0),
    ] {
        assert_eq!(
            composed.surface.alpha_at(x.ceil() as i32, y.ceil() as i32),
            255,
            "the text rectangle's corner ({x}, {y}) lies where the rounded body is opaque"
        );
    }
}

/// A destination near the screen edge must not push the bubble off it: core
/// places the label, and the surface has to follow that placement.
#[test]
fn a_label_near_the_screen_edge_stays_on_screen() {
    let style = CursorOverlayStyle::default();
    let mut edge = frame(&style, Some(pill(80.0)));
    edge.tip = Point {
        x: 1600.0,
        y: 770.0,
    };

    let composed = compose(&edge);
    let text = composed.text_rect.expect("a text rectangle");

    let absolute_right = composed.origin.x + text.x + text.width;
    let absolute_bottom = composed.origin.y + text.y + text.height;
    assert!(
        absolute_right <= screen().width + 1.0,
        "{absolute_right} runs off the right"
    );
    assert!(
        absolute_bottom <= screen().height + 1.0,
        "{absolute_bottom} runs off the bottom"
    );
}
