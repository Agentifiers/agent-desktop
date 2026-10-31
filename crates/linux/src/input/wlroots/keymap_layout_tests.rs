use agent_desktop_core::Modifier;

use super::*;

#[test]
fn the_modifiers_hold_the_first_four_keycodes() {
    let layout = KeymapLayout::default();
    assert_eq!(layout.evdev_code(KEYSYM_SHIFT_L), Some(1));
    assert_eq!(layout.evdev_code(KEYSYM_CONTROL_L), Some(2));
    assert_eq!(layout.evdev_code(KEYSYM_ALT_L), Some(3));
    assert_eq!(layout.evdev_code(KEYSYM_SUPER_L), Some(4));
}

#[test]
fn every_distinct_keysym_gets_its_own_key_once() {
    let layout = KeymapLayout::with_keysyms([0x42, 0x65, 0x42, 0x0100_263a, 0]);
    assert_eq!(layout.evdev_code(0x42), Some(5));
    assert_eq!(layout.evdev_code(0x65), Some(6));
    assert_eq!(layout.evdev_code(0x0100_263a), Some(7));
    assert_eq!(layout.evdev_code(0), None);
    assert_eq!(layout.keysym_at(13), Some(0x42));
}

#[test]
fn the_text_names_each_keysym_in_hex_and_maps_the_real_modifiers() {
    let text = KeymapLayout::with_keysyms([0x42]).text();
    assert!(text.contains("<K13> = 13;"), "{text}");
    assert!(text.contains("key <K13> { [ 0x00000042 ] };"), "{text}");
    assert!(text.contains("key <K9> { [ 0x0000ffe1 ] };"), "{text}");
    assert!(text.contains("modifier_map Shift { <K9> };"), "{text}");
    assert!(text.contains("modifier_map Control { <K10> };"), "{text}");
    assert!(text.contains("modifier_map Mod1 { <K11> };"), "{text}");
    assert!(text.contains("modifier_map Mod4 { <K12> };"), "{text}");
    assert!(text.contains("maximum = 13;"), "{text}");
}

#[test]
fn a_lowercase_letter_carries_its_capital_at_the_shift_level() {
    let text = KeymapLayout::with_keysyms([0x62, 0xe9, 0x0100_0434, 0x31]).text();
    assert!(
        text.contains("key <K13> { [ 0x00000062, 0x00000042 ] };"),
        "{text}"
    );
    assert!(
        text.contains("key <K14> { [ 0x000000e9, 0x000000c9 ] };"),
        "{text}"
    );
    assert!(
        text.contains("key <K15> { [ 0x01000434, 0x01000414 ] };"),
        "{text}"
    );
    assert!(text.contains("key <K16> { [ 0x00000031 ] };"), "{text}");
}

#[test]
fn only_a_single_different_uppercase_form_is_a_shift_level() {
    assert_eq!(shifted(0x61), Some(0x41));
    assert_eq!(shifted(0x41), None);
    assert_eq!(shifted(0x31), None);
    assert_eq!(shifted(0xdf), None);
    assert_eq!(shifted(KEYSYM_SHIFT_L), None);
}

#[test]
fn a_full_layout_replaces_a_key_that_is_not_held() {
    let many: Vec<u32> = (0..400).map(|index| 0x0100_4e00 + index).collect();
    let mut layout = KeymapLayout::with_keysyms(many.iter().copied());
    assert_eq!(layout.evdev_code(many[242]), Some(LAST_KEYCODE - 8));
    assert_eq!(layout.evdev_code(many[243]), None);
    let held = [many[0]];
    let code = layout.admit(many[300], &held).expect("a free slot");
    assert_eq!(layout.evdev_code(many[0]), Some(5));
    assert_eq!(layout.evdev_code(many[1]), None);
    assert_eq!(code, 6);
}

#[test]
fn nothing_is_admitted_when_every_key_is_held() {
    let many: Vec<u32> = (0..400).map(|index| 0x0100_4e00 + index).collect();
    let mut layout = KeymapLayout::with_keysyms(many.iter().copied());
    assert_eq!(layout.admit(0x0100_0001, &many), None);
}

#[test]
fn modifier_masks_follow_the_real_modifier_map() {
    assert_eq!(modifier_mask(KEYSYM_SHIFT_L), 1);
    assert_eq!(modifier_mask(KEYSYM_CONTROL_L), 4);
    assert_eq!(modifier_mask(KEYSYM_ALT_L), 8);
    assert_eq!(modifier_mask(KEYSYM_SUPER_L), 64);
    assert_eq!(modifier_mask(0x61), 0);
}

#[test]
fn the_pointer_plans_find_every_modifier_in_the_keymap_view() {
    let keymap = KeymapLayout::default().as_keymap();
    assert_eq!(keymap.modifier_code(&Modifier::Shift), Some(9));
    assert_eq!(keymap.modifier_code(&Modifier::Ctrl), Some(10));
    assert_eq!(keymap.modifier_code(&Modifier::Alt), Some(11));
    assert_eq!(keymap.modifier_code(&Modifier::Meta), Some(12));
    assert_eq!(keymap.keysyms_of(9), Some(vec![KEYSYM_SHIFT_L]));
}
