/// A request `zwlr_foreign_toplevel_handle_v1` carries to one toplevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToplevelRequest {
    Activate,
    Close,
    SetMinimized(bool),
    SetMaximized(bool),
}
