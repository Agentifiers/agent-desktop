use std::cell::RefCell;
use std::rc::Rc;

use agent_desktop_core::{AdapterError, Deadline};

use crate::system::capture::toplevel_source::ToplevelSource;
use crate::system::wlroots::captured_image::CapturedImage;
use crate::system::wlroots::listed_toplevel::ListedToplevel;
use crate::system::wlroots::output::Output;
use crate::tree::Extents;

pub(crate) fn toplevel(identifier: &str, title: &str, app_id: &str) -> ListedToplevel {
    ListedToplevel {
        identifier: identifier.to_owned(),
        title: title.to_owned(),
        app_id: app_id.to_owned(),
        closed: false,
    }
}

/// A `width` x `height` image whose pixel `(x, y)` is `[x, y, 9, alpha]`,
/// opaque inside `opaque` and with alpha 40 elsewhere.
pub(crate) fn image(width: u32, height: u32, opaque: Option<Extents>) -> CapturedImage {
    let inside = |x: i32, y: i32| {
        opaque.is_some_and(|area| {
            x >= area.x && y >= area.y && x < area.x + area.width && y < area.y + area.height
        })
    };
    let mut rgba = Vec::new();
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let alpha = if inside(x, y) { 255 } else { 40 };
            rgba.extend_from_slice(&[x as u8, y as u8, 9, alpha]);
        }
    }
    CapturedImage {
        width,
        height,
        rgba,
    }
}

/// As [`image`] read from a buffer without alpha: every pixel opaque and the
/// shadow outside `content` plain black.
pub(crate) fn black_margins(width: u32, height: u32, content: Extents) -> CapturedImage {
    let mut flat = image(width, height, Some(content));
    for pixel in flat.rgba.chunks_exact_mut(4) {
        if pixel[3] != 255 {
            pixel.copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    flat
}

pub(crate) fn output(name: &str, x: i32, width: i32, scale: f64) -> Output {
    Output {
        name: name.to_owned(),
        x,
        y: 0,
        width,
        height: 1080,
        scale,
        logical: true,
    }
}

/// What a fake toplevel list saw: the identifiers it copied.
#[derive(Default)]
pub(crate) struct Seen {
    pub(crate) opened: usize,
    pub(crate) copied: Vec<String>,
}

/// A toplevel list that lists `before` until a copy and `after` from then on.
pub(crate) struct FakeToplevels {
    pub(crate) before: Vec<ListedToplevel>,
    pub(crate) after: Vec<ListedToplevel>,
    pub(crate) image: CapturedImage,
    pub(crate) seen: Rc<RefCell<Seen>>,
}

impl ToplevelSource for FakeToplevels {
    fn listed(&self) -> Vec<ListedToplevel> {
        if self.seen.borrow().copied.is_empty() {
            self.before.clone()
        } else {
            self.after.clone()
        }
    }

    fn copy(
        &mut self,
        identifier: &str,
        _deadline: Deadline,
    ) -> Result<CapturedImage, AdapterError> {
        self.seen.borrow_mut().copied.push(identifier.to_owned());
        Ok(self.image.clone())
    }
}

/// A toplevel world: the toplevels listed, the image a copy returns, and
/// what was asked of it.
pub(crate) struct World {
    pub(crate) before: Vec<ListedToplevel>,
    pub(crate) after: Option<Vec<ListedToplevel>>,
    pub(crate) image: CapturedImage,
    pub(crate) seen: Rc<RefCell<Seen>>,
}

impl World {
    pub(crate) fn of(listed: Vec<ListedToplevel>, image: CapturedImage) -> Self {
        Self {
            before: listed,
            after: None,
            image,
            seen: Rc::default(),
        }
    }

    pub(crate) fn open(&self) -> Result<Box<dyn ToplevelSource>, AdapterError> {
        self.seen.borrow_mut().opened += 1;
        Ok(Box::new(FakeToplevels {
            before: self.before.clone(),
            after: self.after.clone().unwrap_or_else(|| self.before.clone()),
            image: self.image.clone(),
            seen: Rc::clone(&self.seen),
        }))
    }

    pub(crate) fn copied(&self) -> Vec<String> {
        self.seen.borrow().copied.clone()
    }

    pub(crate) fn opened(&self) -> usize {
        self.seen.borrow().opened
    }
}

pub(crate) fn no_names(_pid: u32) -> Vec<String> {
    Vec::new()
}
