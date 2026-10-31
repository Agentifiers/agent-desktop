use super::*;

fn with(interfaces: &[&str]) -> Capabilities {
    Capabilities {
        globals: interfaces
            .iter()
            .enumerate()
            .map(|(name, interface)| Global {
                name: name as u32 + 1,
                interface: (*interface).to_owned(),
                version: 1,
            })
            .collect(),
    }
}

#[test]
fn input_needs_a_seat_a_virtual_keyboard_and_a_virtual_pointer() {
    assert!(with(&[SEAT, VIRTUAL_KEYBOARD, VIRTUAL_POINTER]).input());
    assert!(!with(&[VIRTUAL_KEYBOARD, VIRTUAL_POINTER]).input());
    assert!(!with(&[SEAT, VIRTUAL_POINTER]).input());
    assert!(!with(&[SEAT, VIRTUAL_KEYBOARD]).input());
}

#[test]
fn a_mutter_like_global_list_offers_no_wlroots_capability() {
    let mutter = with(&[
        "wl_seat",
        "wl_output",
        "xdg_wm_base",
        "zxdg_output_manager_v1",
    ]);
    assert!(!mutter.input());
    assert!(!mutter.screencopy());
    assert!(!mutter.toplevel_capture());
    assert!(!mutter.toplevel_control());
}

#[test]
fn toplevel_capture_needs_the_list_the_source_and_the_copy_manager() {
    assert!(with(&[TOPLEVEL_LIST, IMAGE_COPY_CAPTURE, TOPLEVEL_CAPTURE_SOURCE]).toplevel_capture());
    assert!(!with(&[TOPLEVEL_LIST, IMAGE_COPY_CAPTURE]).toplevel_capture());
}

#[test]
fn toplevel_control_needs_the_manager_and_a_seat_to_activate_with() {
    assert!(with(&[FOREIGN_TOPLEVEL, SEAT]).toplevel_control());
    assert!(!with(&[FOREIGN_TOPLEVEL]).toplevel_control());
}

#[test]
fn the_cursor_overlay_needs_a_layer_shell_surfaces_and_shared_memory() {
    assert!(with(&[LAYER_SHELL, COMPOSITOR, SHM]).cursor_overlay());
    assert!(!with(&[COMPOSITOR, SHM]).cursor_overlay());
    assert!(!with(&[LAYER_SHELL, SHM]).cursor_overlay());
    assert!(!with(&[LAYER_SHELL, COMPOSITOR]).cursor_overlay());
}

#[test]
fn the_cache_key_names_both_the_runtime_dir_and_the_display() {
    let key = session_key();
    assert!(key.contains('\u{0}'));
}
