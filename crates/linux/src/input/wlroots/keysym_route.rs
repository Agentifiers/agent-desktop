//! Keys and text on a wlroots compositor through a virtual keyboard whose
//! keymap is generated for the keysyms of the delivery.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::virtual_devices::VirtualDevices;
use super::virtual_key_session::VirtualKeySession;
use crate::input::delivery::{Delivery, Gate};
use crate::input::keyboard::keysym_step::KeysymStep;
use crate::input::wayland::keysym_send::run_chunks;

const UNICODE_KEYSYM_BASE: u32 = 0x0100_0000;

/// Whether `keysym` is a Unicode control character, which no key types.
fn is_control(keysym: u32) -> bool {
    keysym
        .checked_sub(UNICODE_KEYSYM_BASE)
        .and_then(char::from_u32)
        .is_some_and(char::is_control)
}

pub(crate) fn refuse_controls(keys: &[KeysymStep]) -> Result<(), AdapterError> {
    if !keys.iter().any(|key| is_control(key.keysym)) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The text holds a control character that no key types",
    )
    .with_suggestion("Remove the control character, or use the semantic set-value on the element")
    .with_details(json!({
        "physical_delivery_started": false,
        "cause": "control_character",
    }))
    .with_disposition(DeliverySemantics::not_delivered()))
}

pub(crate) fn send_chunks(
    chunks: &[Vec<KeysymStep>],
    gate: Gate<'_>,
    deadline: Deadline,
) -> Result<Delivery, AdapterError> {
    for chunk in chunks {
        refuse_controls(chunk)?;
    }
    let chunks: Vec<&[KeysymStep]> = chunks
        .iter()
        .map(Vec::as_slice)
        .filter(|chunk| !chunk.is_empty())
        .collect();
    if chunks.is_empty() {
        return Ok(Delivery {
            delivered_events: 0,
        });
    }
    let keysyms = chunks
        .iter()
        .flat_map(|chunk| chunk.iter().map(|key| key.keysym));
    let mut session =
        VirtualKeySession::new(|deadline| VirtualDevices::open(false, deadline), keysyms);
    run_chunks(&mut session, &chunks, gate, deadline)
}

#[cfg(test)]
#[path = "keysym_route_tests.rs"]
mod tests;
