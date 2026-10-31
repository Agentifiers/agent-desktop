//! Live checks against a running wlroots compositor (a private headless
//! labwc session). They print each toplevel's identifier so an independent
//! capture (`grim -T <identifier>`) can be compared with this tool's.

use agent_desktop_core::Deadline;

use crate::system::capture::wlroots_display::copy_output;
use crate::system::capture::wlroots_unit::content_box;
use crate::system::wlroots::toplevel_capture::ToplevelLister;

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("a deadline builds")
}

#[test]
#[ignore = "needs a private wlroots session with a window open; run inside a headless labwc"]
fn wlroots_live_every_toplevel_is_listed_and_copied_with_its_content_box() {
    let mut lister = ToplevelLister::open(deadline()).expect("the toplevel list opens");
    let listed = lister.toplevels();
    assert!(!listed.is_empty(), "no toplevel is open");
    for toplevel in listed {
        let image = lister
            .capture(&toplevel.identifier, deadline())
            .expect("the toplevel is copied");
        println!(
            "identifier={} title={:?} app_id={:?} surface={}x{} content={:?}",
            toplevel.identifier,
            toplevel.title,
            toplevel.app_id,
            image.width,
            image.height,
            content_box(&image),
        );
    }
}

#[test]
#[ignore = "needs a private wlroots session; run inside a headless labwc"]
fn wlroots_live_the_first_output_is_copied_at_its_mode_size() {
    let outputs = crate::system::wlroots::outputs::list(deadline()).expect("outputs list");
    let first = outputs.first().expect("an output");
    let (output, image) = copy_output(&first.name, deadline()).expect("the output is copied");
    assert_eq!(output.name, first.name);
    assert_eq!(
        (f64::from(image.width), f64::from(image.height)),
        (
            output.rect().width * output.scale.max(1.0),
            output.rect().height * output.scale.max(1.0)
        )
    );
}
