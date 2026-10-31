/// One toplevel as `ext_foreign_toplevel_list_v1` describes it. The
/// identifier is stable for the toplevel's life and names it to a capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ListedToplevel {
    pub(crate) identifier: String,
    pub(crate) title: String,
    pub(crate) app_id: String,
    pub(crate) closed: bool,
}
