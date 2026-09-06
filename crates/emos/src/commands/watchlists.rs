mod genre;
mod hot;
mod scifi;
mod trending;

pub(crate) use genre::update_by_genre;
pub(crate) use hot::generate_hot_watchlist;
pub(crate) use scifi::generate_scifi_watchlist;
pub(crate) use trending::generate_trending_watchlist;
