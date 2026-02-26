use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Simple cache error: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("{0} (other cache err)")]
    Other(String),
}
