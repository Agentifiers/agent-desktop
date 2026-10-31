//! Ties an accessibility window to a compositor toplevel. Neither wlroots
//! toplevel protocol names a process, so the tie rests on the window title,
//! which must be equal, and on the application id, which only separates
//! toplevels that share a title. Two windows of one application with one
//! title stay ambiguous.

/// What the accessibility tree says about the window: its title and every
/// name its application is known by (desktop id, executable name).
#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowIdentity<'a> {
    pub(crate) title: &'a str,
    pub(crate) app_names: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToplevelMatch {
    Unique(usize),
    Ambiguous(usize),
    Missing,
}

fn same_app(app_id: &str, names: &[String]) -> bool {
    let id = app_id.to_ascii_lowercase();
    let short = id.rsplit('.').next().unwrap_or(&id).to_owned();
    names.iter().any(|name| {
        let name = name.to_ascii_lowercase();
        let name = name.strip_suffix(".desktop").unwrap_or(&name);
        name == id || name == short || name.rsplit('.').next() == Some(short.as_str())
    })
}

/// The toplevel among `toplevels` (title, app id) that is `window`.
pub(crate) fn match_toplevel<'a>(
    toplevels: impl IntoIterator<Item = (&'a str, &'a str)>,
    window: WindowIdentity<'_>,
) -> ToplevelMatch {
    let titled: Vec<(usize, &str)> = toplevels
        .into_iter()
        .enumerate()
        .filter(|(_, (title, _))| *title == window.title)
        .map(|(index, (_, app_id))| (index, app_id))
        .collect();
    match titled.as_slice() {
        [] => return ToplevelMatch::Missing,
        [(only, _)] => return ToplevelMatch::Unique(*only),
        _ => {}
    }
    let same: Vec<usize> = titled
        .iter()
        .filter(|(_, app_id)| same_app(app_id, window.app_names))
        .map(|(index, _)| *index)
        .collect();
    match same.as_slice() {
        [only] => ToplevelMatch::Unique(*only),
        [] => ToplevelMatch::Ambiguous(titled.len()),
        many => ToplevelMatch::Ambiguous(many.len()),
    }
}

/// Every name the application of `pid` is known by: its listed executable
/// name and the desktop entry names and ids its executable answers to.
pub(crate) fn app_names(pid: u32) -> Vec<String> {
    let Some(facts) = crate::system::process::process_facts::read(pid) else {
        return Vec::new();
    };
    let mut names = vec![crate::system::app::app_name::app_name(&facts)];
    names.extend(crate::system::app::app_name::aliases_of(&facts));
    names
}

#[cfg(test)]
#[path = "toplevel_match_tests.rs"]
mod tests;
