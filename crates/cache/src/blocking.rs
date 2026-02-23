use std::fmt::Display;

use tracing::instrument;

#[instrument(level = "debug", skip_all, fields(label = %_label))]
pub fn block_in_place<F, R>(_label: impl Display, cb: F) -> R
where
    F: FnOnce() -> R,
{
    tokio::task::block_in_place(cb)
}
