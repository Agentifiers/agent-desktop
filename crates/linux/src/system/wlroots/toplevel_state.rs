use super::toplevel_flags::ToplevelFlags;

/// One toplevel as `zwlr_foreign_toplevel_handle_v1` describes it after its
/// latest `done`. `key` names the toplevel to requests on the connection that
/// read it; it means nothing on another connection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ToplevelState {
    pub(crate) key: u32,
    pub(crate) title: String,
    pub(crate) app_id: String,
    pub(crate) flags: ToplevelFlags,
    /// The names of the outputs the toplevel is on.
    pub(crate) outputs: Vec<String>,
    /// Whether the toplevel is the child of another, as a dialog is.
    pub(crate) has_parent: bool,
}
