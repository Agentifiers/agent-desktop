/// Where the overlay is in going to rest after a quiet spell: awake, fading
/// out since an instant, or gone until the next control, as on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rest {
    Awake,
    Fading(u64),
    Resting,
}
