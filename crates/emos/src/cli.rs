use anyhow::Context;
use clap::Parser;
use clap::Subcommand;

use crate::commands;

#[derive(Parser)]
#[command(
    name = "emos",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(name = "tmdb_scifi_media")]
    Scifi {
        #[arg(long = "flag")]
        should_download_posters: bool,
    },
    #[command(name = "watch_basic_genre")]
    Genre {
        #[arg(long, value_name = "genre")]
        genre: String,
        #[arg(long = "watch_id", value_name = "watch_id")]
        watch_id: String,
    },
    #[command(name = "trakt_auth")]
    Auth,
    #[command(name = "trakt_trending")]
    Trending {
        #[arg(long = "download_cover")]
        download_cover: bool,
    },
    #[command(name = "watch_hot_video")]
    Hot {
        #[arg(long = "douban_user_id", value_name = "douban_user_id")]
        douban_user_id: Option<String>,
    },
    #[command(name = "sync_video_list")]
    Sync,
    #[command(name = "tmdb_download_cover")]
    Covers {
        #[arg(long)]
        video: bool,
        #[arg(long = "id", value_name = "id", required = true)]
        tmdb_id: Vec<String>,
        #[arg(long, value_name = "namespace")]
        namespace: String,
    },
}

impl Cli {
    pub(crate) async fn run(self) {
        match self.command {
            Command::Scifi {
                should_download_posters,
            } => commands::generate_scifi_watchlist(should_download_posters)
                .await
                .context("tmdb_scifi_media"),
            Command::Genre { genre, watch_id } => commands::update_by_genre(genre, watch_id)
                .await
                .context("watch_basic_genre"),
            Command::Auth => commands::authorize_trakt().await.context("trakt_auth"),
            Command::Trending { download_cover } => {
                commands::generate_trending_watchlist(download_cover)
                    .await
                    .context("trakt_trending")
            }
            Command::Hot { douban_user_id } => commands::generate_hot_watchlist(douban_user_id)
                .await
                .context("watch_hot_video"),
            Command::Sync => commands::sync_videos().await.context("sync_video_list"),
            Command::Covers {
                video,
                tmdb_id,
                namespace,
            } => commands::download_covers(video, tmdb_id, namespace)
                .await
                .context("tmdb_download_cover"),
        }
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn registered_commands_form_a_valid_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cover_command_parses_renamed_repeated_ids_and_flag() {
        let cli = Cli::try_parse_from([
            "emos",
            "tmdb_download_cover",
            "--video",
            "--id",
            "101",
            "--id",
            "202",
            "--namespace",
            "test",
        ])
        .unwrap();
        let Command::Covers {
            video,
            tmdb_id,
            namespace,
        } = cli.command
        else {
            panic!("expected cover command");
        };

        assert!(video);
        assert_eq!(tmdb_id, ["101", "202"]);
        assert_eq!(namespace, "test");
    }

    #[test]
    fn cover_command_requires_an_id() {
        let error = Cli::command()
            .try_get_matches_from(["emos", "tmdb_download_cover", "--namespace", "test"])
            .unwrap_err();

        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn hot_watchlist_accepts_an_optional_douban_user() {
        let cli = Cli::try_parse_from(["emos", "watch_hot_video"]).unwrap();
        let Command::Hot { douban_user_id } = cli.command else {
            panic!("expected hot watchlist command");
        };
        assert_eq!(douban_user_id, None);

        let cli =
            Cli::try_parse_from(["emos", "watch_hot_video", "--douban_user_id", "123"]).unwrap();
        let Command::Hot { douban_user_id } = cli.command else {
            panic!("expected hot watchlist command");
        };
        assert_eq!(douban_user_id.as_deref(), Some("123"));
    }
}
