use agent_desktop_core::CursorOverlayConfig;
use agent_desktop_core::session::{ArtifactsMode, SessionManifest, SessionTraceMode};

use super::admits_manifest;

fn manifest(overlay: CursorOverlayConfig, ended: bool) -> SessionManifest {
    SessionManifest {
        id: "run-1".into(),
        name: None,
        created_at: 1,
        ended_at: ended.then_some(2),
        trace: SessionTraceMode::Off,
        artifacts: ArtifactsMode::Events,
        cursor_overlay: overlay,
    }
}

fn enabled() -> CursorOverlayConfig {
    CursorOverlayConfig::enabled(None, 6).expect("config")
}

#[test]
fn a_live_enabled_session_admits_its_cursor() {
    assert!(admits_manifest(Some(&manifest(enabled(), false)), None));
    assert!(admits_manifest(
        Some(&manifest(enabled(), false)),
        Some("a")
    ));
}

#[test]
fn ended_disabled_or_missing_sessions_admit_nothing() {
    assert!(!admits_manifest(Some(&manifest(enabled(), true)), None));
    assert!(!admits_manifest(
        Some(&manifest(CursorOverlayConfig::default(), false)),
        None
    ));
    assert!(!admits_manifest(None, Some("a")));
}

#[test]
fn a_multi_agent_session_admits_only_named_agents() {
    let multi = manifest(enabled().with_multi_agent(true), false);
    assert!(!admits_manifest(Some(&multi), None));
    assert!(admits_manifest(Some(&multi), Some("agent-a")));
}
