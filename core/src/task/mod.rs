use std::pin::Pin;

pub(crate) mod sync_video_list;
mod tmdb_download_cover;
mod tmdb_scifi_media;
pub(crate) mod watch_basic_genre;
pub(crate) mod watch_hot_video;

pub type TaskFn = fn(&clap::ArgMatches) -> Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ArgKind {
    Required,
    Optional,
    Many,
    Flag,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskArg {
    pub(crate) name: &'static str,
    pub(crate) kind: ArgKind,
}

pub(crate) struct Task {
    pub(crate) name: &'static str,
    pub(crate) args: &'static [TaskArg],
    pub(crate) run: TaskFn,
}

inventory::collect!(Task);
