mod auth;
mod covers;
mod library;
mod watchlists;

pub(crate) use auth::authorize_trakt;
pub(crate) use covers::download_covers;
pub(crate) use library::sync_videos;
pub(crate) use watchlists::generate_hot_watchlist;
pub(crate) use watchlists::generate_scifi_watchlist;
pub(crate) use watchlists::generate_trending_watchlist;
pub(crate) use watchlists::update_by_genre;
