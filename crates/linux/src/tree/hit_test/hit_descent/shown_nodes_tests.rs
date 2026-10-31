use super::*;

fn words(bits: &[u32]) -> Vec<u32> {
    let mut words = vec![0_u32, 0_u32];
    for bit in bits {
        words[(*bit / 32) as usize] |= 1 << (bit % 32);
    }
    words
}

#[test]
fn a_node_neither_visible_nor_showing_is_not_drawn() {
    assert!(!drawn(Some(&words(&[8, 24]))));
}

#[test]
fn either_visibility_bit_keeps_the_node() {
    assert!(drawn(Some(&words(&[VISIBLE]))));
    assert!(drawn(Some(&words(&[SHOWING]))));
    assert!(drawn(Some(&words(&[VISIBLE, SHOWING]))));
}

#[test]
fn an_unread_state_rules_nothing_out() {
    assert!(drawn(None));
}
