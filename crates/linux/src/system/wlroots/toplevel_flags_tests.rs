use super::*;

fn array(states: &[u32]) -> Vec<u8> {
    states
        .iter()
        .flat_map(|state| state.to_ne_bytes())
        .collect()
}

#[test]
fn every_known_state_is_read_from_the_array() {
    let flags = ToplevelFlags::from_state_array(&array(&[0, 1, 2, 3]));
    assert_eq!(
        flags,
        ToplevelFlags {
            activated: true,
            maximized: true,
            minimized: true,
            fullscreen: true,
        }
    );
}

#[test]
fn an_empty_array_is_a_plain_toplevel_and_unknown_states_are_ignored() {
    assert_eq!(
        ToplevelFlags::from_state_array(&[]),
        ToplevelFlags::default()
    );
    let flags = ToplevelFlags::from_state_array(&array(&[2, 99]));
    assert!(flags.activated);
    assert!(!flags.maximized && !flags.minimized && !flags.fullscreen);
}

#[test]
fn a_trailing_partial_word_is_ignored() {
    let mut bytes = array(&[1]);
    bytes.push(0);
    assert!(ToplevelFlags::from_state_array(&bytes).minimized);
}
