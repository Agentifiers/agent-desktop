use agent_desktop_core::ErrorCode;

use super::*;
use crate::input::keyboard::text_plan::plan_text_keysyms;

#[test]
fn printable_latin_and_unicode_text_is_never_refused() {
    for chunk in plan_text_keysyms("Ben from srv! @#$ é 你 ☺") {
        refuse_controls(&chunk).expect("typable");
    }
}

#[test]
fn a_control_character_other_than_newline_and_tab_is_refused_before_any_key() {
    let chunks = plan_text_keysyms("a\u{7}b");
    let error = refuse_controls(&chunks[0]).expect_err("bell");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "control_character");
    assert_eq!(details["physical_delivery_started"], false);
    for chunk in plan_text_keysyms("a\nb\tc") {
        refuse_controls(&chunk).expect("newline and tab are keys");
    }
}

#[test]
fn empty_text_sends_nothing_and_opens_no_device() {
    let delivery = send_chunks(
        &[Vec::new()],
        &mut || Ok(()),
        Deadline::after(1_000).unwrap(),
    )
    .expect("nothing to send");
    assert_eq!(delivery.delivered_events, 0);
}
