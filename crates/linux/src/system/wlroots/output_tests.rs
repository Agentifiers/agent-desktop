use super::*;

fn output(name: &str, x: i32, y: i32, width: i32, height: i32) -> Output {
    Output {
        name: name.to_owned(),
        x,
        y,
        width,
        height,
        scale: 1.0,
        logical: true,
    }
}

#[test]
fn the_layout_spans_every_output() {
    let outputs = [
        output("HEADLESS-1", 0, 0, 1280, 720),
        output("HEADLESS-2", 1280, -100, 800, 600),
    ];
    let bounds = layout_bounds(&outputs).expect("bounds");
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (0.0, -100.0, 2080.0, 820.0)
    );
}

#[test]
fn no_sized_output_gives_no_layout() {
    assert!(layout_bounds(&[output("A", 0, 0, 0, 0)]).is_none());
    assert!(layout_bounds(&[]).is_none());
}

#[test]
fn a_mode_without_a_logical_size_is_divided_by_the_scale() {
    let scaled = Output {
        scale: 2.0,
        logical: false,
        ..output("eDP-1", 0, 0, 2560, 1440)
    };
    assert_eq!(scaled.rect().width, 1280.0);
    assert_eq!(scaled.rect().height, 720.0);
}

#[test]
fn the_output_at_the_origin_is_primary_and_listed_first() {
    let displays = to_displays(&[output("B", 1280, 0, 800, 600), output("A", 0, 0, 1280, 720)]);
    assert_eq!(displays[0].id, "A");
    assert!(displays[0].is_primary);
    assert!(!displays[1].is_primary);
}

#[test]
fn without_an_output_at_the_origin_the_first_is_primary() {
    let displays = to_displays(&[output("B", 10, 0, 800, 600), output("A", 900, 0, 10, 10)]);
    assert_eq!(displays[0].id, "B");
    assert!(displays[0].is_primary);
}
