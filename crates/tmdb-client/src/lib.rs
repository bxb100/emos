mod client;
pub mod media;
pub mod movie;
pub mod tv;

pub use client::Client;

pub const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/original";
