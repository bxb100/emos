use std::pin::Pin;

pub(crate) mod sync_video_list;
mod tmdb_scifi_media;
pub(crate) mod watch_basic_genre;
mod watch_hot_and_persistent;
pub(crate) mod watch_hot_video;

pub type TaskFn = fn(&clap::ArgMatches) -> Pin<Box<dyn Future<Output = ()> + Send>>;

pub(crate) struct Task {
    pub(crate) name: &'static str,
    pub(crate) args: &'static [&'static str],
    pub(crate) run: TaskFn,
}

inventory::collect!(Task);
