//! The XKB keymap a virtual keyboard uploads: every keysym a delivery needs
//! sits on its own keycode at the first level, so any character, shifted or
//! not, Latin-1 or Unicode, is one key with no layout lookup. A lowercase
//! letter's key carries its uppercase form at the Shift level, so a chord
//! such as `shift+b` reads as it would from a real keyboard. Shift, Control,
//! Alt and Super always hold the first four keycodes and the modifier map,
//! so a chord's modifier state is exact.

use std::fmt::Write as _;

use crate::input::keyboard::key_names::{
    KEYSYM_ALT_L, KEYSYM_CONTROL_L, KEYSYM_SHIFT_L, KEYSYM_SUPER_L, keysym_for_char,
};
use crate::input::keyboard::keymap::Keymap;

/// The first X keycode the layout uses; an evdev code is the X keycode
/// minus eight.
pub(crate) const FIRST_KEYCODE: u32 = 9;
/// X clients, Xwayland's included, cannot address keycodes past 255.
pub(crate) const LAST_KEYCODE: u32 = 255;
const EVDEV_OFFSET: u32 = 8;
const UNICODE_KEYSYM_BASE: u32 = 0x0100_0000;

const SHIFT_MASK: u32 = 1;
const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3;
const MOD4_MASK: u32 = 1 << 6;
const MOD5_MASK: u32 = 1 << 7;

const MODIFIERS: [(u32, &str); 4] = [
    (KEYSYM_SHIFT_L, "Shift"),
    (KEYSYM_CONTROL_L, "Control"),
    (KEYSYM_ALT_L, "Mod1"),
    (KEYSYM_SUPER_L, "Mod4"),
];

/// The real modifier a keysym sets while its key is down, as a
/// `wl_keyboard.modifiers` depressed mask bit; zero for an ordinary key.
pub(crate) fn modifier_mask(keysym: u32) -> u32 {
    match keysym {
        0xffe1 | 0xffe2 => SHIFT_MASK,
        0xffe3 | 0xffe4 => CONTROL_MASK,
        0xffe7..=0xffea => MOD1_MASK,
        0xffeb..=0xffee => MOD4_MASK,
        0xfe03 => MOD5_MASK,
        _ => 0,
    }
}

/// The keysym Shift gives on a lowercase letter's key: its uppercase form,
/// so `shift+b` types `B` as it does on a real keyboard. `None` for a keysym
/// with no single uppercase counterpart.
pub(crate) fn shifted(keysym: u32) -> Option<u32> {
    let character = match keysym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(keysym)?,
        _ => char::from_u32(keysym.checked_sub(UNICODE_KEYSYM_BASE)?)?,
    };
    let mut upper = character.to_uppercase();
    let (Some(only), None) = (upper.next(), upper.next()) else {
        return None;
    };
    (only != character).then(|| keysym_for_char(only))
}

/// Keysyms by keycode, from [`FIRST_KEYCODE`] upwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeymapLayout {
    keysyms: Vec<u32>,
}

impl Default for KeymapLayout {
    fn default() -> Self {
        Self {
            keysyms: MODIFIERS.iter().map(|(keysym, _)| *keysym).collect(),
        }
    }
}

fn capacity() -> usize {
    (LAST_KEYCODE - FIRST_KEYCODE + 1) as usize
}

impl KeymapLayout {
    /// A layout holding the modifiers and then each distinct keysym of
    /// `keysyms`, as many as fit.
    pub(crate) fn with_keysyms(keysyms: impl IntoIterator<Item = u32>) -> Self {
        let mut layout = Self::default();
        for keysym in keysyms {
            if layout.keysyms.len() >= capacity() {
                break;
            }
            if keysym != 0 && !layout.keysyms.contains(&keysym) {
                layout.keysyms.push(keysym);
            }
        }
        layout
    }

    /// The evdev code of `keysym`'s key, if the layout holds it.
    pub(crate) fn evdev_code(&self, keysym: u32) -> Option<u32> {
        let index = self.keysyms.iter().position(|held| *held == keysym)?;
        Some(FIRST_KEYCODE + index as u32 - EVDEV_OFFSET)
    }

    /// The keysym on the key with X keycode `keycode`.
    pub(crate) fn keysym_at(&self, keycode: u8) -> Option<u32> {
        let index = u32::from(keycode).checked_sub(FIRST_KEYCODE)?;
        self.keysyms.get(index as usize).copied()
    }

    /// Adds `keysym`, replacing the first keysym that is neither a modifier
    /// nor in `keep` once the layout is full. `None` when nothing can be
    /// replaced.
    pub(crate) fn admit(&mut self, keysym: u32, keep: &[u32]) -> Option<u32> {
        if let Some(code) = self.evdev_code(keysym) {
            return Some(code);
        }
        if self.keysyms.len() < capacity() {
            self.keysyms.push(keysym);
        } else {
            let slot = self
                .keysyms
                .iter()
                .enumerate()
                .skip(MODIFIERS.len())
                .find(|(_, held)| !keep.contains(held))
                .map(|(index, _)| index)?;
            self.keysyms[slot] = keysym;
        }
        self.evdev_code(keysym)
    }

    /// The keymap in XKB text form, as `zwp_virtual_keyboard_v1.keymap`
    /// takes it.
    pub(crate) fn text(&self) -> String {
        let last = FIRST_KEYCODE + self.keysyms.len() as u32 - 1;
        let mut text = String::from("xkb_keymap {\nxkb_keycodes \"agent-desktop\" {\n");
        let _ = writeln!(text, "minimum = 8;\nmaximum = {};", last.max(FIRST_KEYCODE));
        for index in 0..self.keysyms.len() as u32 {
            let code = FIRST_KEYCODE + index;
            let _ = writeln!(text, "<K{code}> = {code};");
        }
        text.push_str("};\nxkb_types \"agent-desktop\" { include \"complete\" };\n");
        text.push_str("xkb_compatibility \"agent-desktop\" { include \"complete\" };\n");
        text.push_str("xkb_symbols \"agent-desktop\" {\n");
        for (index, keysym) in self.keysyms.iter().enumerate() {
            let code = FIRST_KEYCODE + index as u32;
            match shifted(*keysym) {
                Some(upper) => {
                    let _ = writeln!(
                        text,
                        "key <K{code}> {{ [ {keysym:#010x}, {upper:#010x} ] }};"
                    );
                }
                None => {
                    let _ = writeln!(text, "key <K{code}> {{ [ {keysym:#010x} ] }};");
                }
            }
        }
        for (index, (_, real)) in MODIFIERS.iter().enumerate() {
            let code = FIRST_KEYCODE + index as u32;
            let _ = writeln!(text, "modifier_map {real} {{ <K{code}> }};");
        }
        text.push_str("};\n};\n");
        text
    }

    /// The layout as an X-style keymap, so the pointer plans can find their
    /// modifier keycodes the way they do on X11.
    pub(crate) fn as_keymap(&self) -> Keymap {
        let first = FIRST_KEYCODE as u8;
        let mut modifier_codes = vec![0_u8; 8];
        modifier_codes[0] = first;
        modifier_codes[2] = first + 1;
        modifier_codes[3] = first + 2;
        modifier_codes[6] = first + 3;
        Keymap::new(first, 1, self.keysyms.clone(), 1, modifier_codes)
    }
}

#[cfg(test)]
#[path = "keymap_layout_tests.rs"]
mod tests;
