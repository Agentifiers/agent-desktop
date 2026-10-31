//! One agent's frame: the Windows composition, the label written with the
//! system face, the card dimmed by its reveal and everything by the rest fade.

use agent_desktop_core::{Point, Rect};

use super::look::raster::Surface;
use super::look::{fade, geometry, render, text};
use super::presentation::Presentation;

pub(crate) struct Light<'a> {
    pub(crate) now_ms: u64,
    pub(crate) overlay_opacity: f64,
    pub(crate) font: Option<&'a fontdue::Font>,
}

pub(crate) fn frame(
    presentation: &Presentation,
    screen: Rect,
    light: &Light<'_>,
) -> Option<(Point, Surface)> {
    let tip = presentation.pose.clone()?;
    let flourish = presentation
        .effect
        .as_ref()
        .and_then(|effect| Some((effect.sample(light.now_ms)?, effect.target)));
    let ((ripple_phase, highlight_opacity), target) = flourish.unwrap_or(((0.0, 0.0), None));
    let label = light.font.and(presentation.label.as_deref());
    let style = &presentation.style;
    let mut composed = render::compose(&render::Frame {
        tip,
        style,
        ripple_phase,
        target,
        highlight_opacity,
        label,
        screen,
    });
    if let (Some(text_rect), Some(label), Some(font)) = (composed.text_rect, label, light.font) {
        let px = geometry::BUBBLE_FONT_POINTS * style.size();
        text::draw_label(
            &mut composed.surface,
            &text_rect,
            label,
            style.rim_rgb(),
            (px, font),
        );
    }
    if let Some(card) = composed.card_rect {
        fade::dim_region(&mut composed.surface, &card, presentation.reveal.opacity());
    }
    fade::dim_surface(&mut composed.surface, light.overlay_opacity);
    Some((composed.origin, composed.surface))
}
