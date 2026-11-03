use super::*;

fn frame() -> AtspiElement {
    AtspiElement::new(":1.3", "/org/gnome/Calculator/a11y/frame", 41)
}

fn deadline() -> Deadline {
    Deadline::after(1_000).expect("deadline")
}

#[test]
fn a_frame_without_a_title_or_rectangle_is_never_placed() {
    let window = Some(Extents {
        x: 0,
        y: 0,
        width: 360,
        height: 616,
    });
    assert!(
        prove_place(&frame(), window, None, deadline())
            .unwrap()
            .is_none()
    );
    assert!(
        prove_place(&frame(), window, Some(""), deadline())
            .unwrap()
            .is_none()
    );
    assert!(
        prove_place(&frame(), None, Some("Calculator"), deadline())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        layout_of(&frame(), None, Some("Calculator"), deadline()),
        LayoutPlace::Unproven
    );
}

#[test]
fn the_unproven_origin_refusal_names_the_window_and_sends_nothing() {
    let error = origin_unproven(Some(&frame()));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "wlroots_window_origin_unproven");
    assert_eq!(details["physical_delivery_started"], false);
    let id = details["window_id"].as_str().expect("window id");
    let suggestion = error.suggestion.expect("suggestion");
    assert!(suggestion.contains(&format!("maximize --window-id {id}")));
    assert!(suggestion.contains(&format!("focus-window --window-id {id}")));
    assert!(suggestion.contains("without --headed"));
    assert!(suggestion.contains("mouse-click --xy"));
    assert!(details.get("check").is_none());
}

#[test]
fn the_refusal_without_a_frame_keeps_the_placeholder() {
    let error = origin_unproven(None);
    let details = error.details.expect("details");
    assert!(details["window_id"].is_null());
    assert!(
        error
            .suggestion
            .expect("suggestion")
            .contains("maximize --window-id <id>")
    );
}

#[test]
fn the_unproven_origin_is_settled_not_waited_out() {
    assert!(!origin_unproven(None).permits_retry_by_default());
}

fn rect(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_node_in_a_stack_page_is_drawn_back_by_the_page_offset() {
    let page = Reported {
        scale: 1.0,
        shift: (12, 12),
    };
    assert_eq!(
        drawn_rect(Some(rect(314, 532, 132, 44)), || Some(page)),
        Some(Some(rect(302, 520, 132, 44)))
    );
    assert_eq!(
        drawn_rect(Some(rect(302, 520, 132, 44)), || Some(Reported::DRAWN)),
        Some(Some(rect(302, 520, 132, 44)))
    );
}

#[test]
fn an_unread_frame_leaves_the_node_unplaced() {
    assert_eq!(drawn_rect(Some(rect(314, 532, 132, 44)), || None), None);
}

#[test]
fn no_rectangle_reads_no_frame() {
    let read = std::cell::Cell::new(false);
    let drawn = drawn_rect(None, || {
        read.set(true);
        Some(Reported::DRAWN)
    });
    assert_eq!(drawn, Some(None));
    assert!(!read.get());
}
