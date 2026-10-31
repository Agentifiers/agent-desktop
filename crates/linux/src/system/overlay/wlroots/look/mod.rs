//! How the overlay looks: the dart, the click ripple, the target highlight
//! and the label card, composed on one premultiplied surface per frame. The
//! composition is the Windows renderer's, carried across unchanged so both
//! software renderers draw the same pixels; only the text is drawn here with
//! a pure-Rust rasterizer instead of GDI.

pub(crate) mod fade;
pub(crate) mod font;
pub(crate) mod geometry;
pub(crate) mod label;
pub(crate) mod raster;
pub(crate) mod render;
pub(crate) mod reveal;
pub(crate) mod rounded;
pub(crate) mod schedule;
pub(crate) mod text;
