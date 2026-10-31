//! Whether a session still wants its overlay: a helper is started only for
//! a live session whose overlay is enabled, and stops once that ends. A
//! multi-agent session draws only agents' cursors, never an anonymous one.

use agent_desktop_core::session::{SessionManifest, read_manifest};

fn wants(manifest: Option<&SessionManifest>) -> bool {
    manifest
        .is_some_and(|manifest| manifest.ended_at.is_none() && manifest.cursor_overlay.is_enabled())
}

/// Whether a helper may start for `agent_id` of the session `manifest`
/// describes.
pub(crate) fn admits_manifest(manifest: Option<&SessionManifest>, agent_id: Option<&str>) -> bool {
    wants(manifest)
        && manifest.is_some_and(|manifest| {
            !(manifest.cursor_overlay.is_multi_agent() && agent_id.is_none())
        })
}

pub(crate) fn admits(session_id: &str, agent_id: Option<&str>) -> bool {
    read_manifest(session_id)
        .ok()
        .flatten()
        .is_some_and(|manifest| admits_manifest(Some(&manifest), agent_id))
}

/// Whether a running helper should keep drawing for `session_id`. A
/// manifest that cannot be read, including one whose state root is gone,
/// ends the helper.
pub(crate) fn serves(session_id: &str) -> bool {
    read_manifest(session_id)
        .ok()
        .flatten()
        .is_some_and(|manifest| wants(Some(&manifest)))
}

#[cfg(test)]
#[path = "session_gate_tests.rs"]
mod tests;
