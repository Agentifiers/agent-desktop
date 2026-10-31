use super::*;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_unique_title_matches_whatever_the_app_id() {
    let toplevels = [
        ("Calculator", "org.gnome.Calculator"),
        ("Click Probe - Chromium", "chromium"),
    ];
    let app = names(&["chromium"]);
    let window = WindowIdentity {
        title: "Click Probe - Chromium",
        app_names: &app,
    };
    assert_eq!(match_toplevel(toplevels, window), ToplevelMatch::Unique(1));
}

#[test]
fn no_equal_title_is_missing() {
    let app = names(&["gedit"]);
    let window = WindowIdentity {
        title: "Untitled",
        app_names: &app,
    };
    assert_eq!(
        match_toplevel([("Untitled 2", "gedit")], window),
        ToplevelMatch::Missing
    );
}

#[test]
fn a_shared_title_is_split_by_the_application_id() {
    let toplevels = [
        ("New", "org.gnome.TextEditor"),
        ("New", "org.gnome.Nautilus"),
    ];
    let app = names(&["gnome-text-editor", "org.gnome.TextEditor.desktop"]);
    let window = WindowIdentity {
        title: "New",
        app_names: &app,
    };
    assert_eq!(match_toplevel(toplevels, window), ToplevelMatch::Unique(0));
}

#[test]
fn the_short_id_matches_an_executable_name() {
    let toplevels = [("Files", "org.gnome.Nautilus"), ("Files", "foot")];
    let app = names(&["nautilus"]);
    let window = WindowIdentity {
        title: "Files",
        app_names: &app,
    };
    assert_eq!(match_toplevel(toplevels, window), ToplevelMatch::Unique(0));
}

#[test]
fn two_windows_of_one_app_with_one_title_stay_ambiguous() {
    let toplevels = [
        ("Calculator", "org.gnome.Calculator"),
        ("Calculator", "org.gnome.Calculator"),
    ];
    let app = names(&["org.gnome.Calculator"]);
    let window = WindowIdentity {
        title: "Calculator",
        app_names: &app,
    };
    assert_eq!(
        match_toplevel(toplevels, window),
        ToplevelMatch::Ambiguous(2)
    );
}
