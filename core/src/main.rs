use std::pin::Pin;

use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::Command;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::task::Task;

mod task;

fn build_cli() -> Command {
    let mut cmd = Command::new("core")
        .subcommand_required(true)
        .arg_required_else_help(true);

    for task in inventory::iter::<Task> {
        let mut sub = Command::new(task.name);

        for &arg_name in task.args {
            sub = sub.arg(
                Arg::new(arg_name)
                    .long(arg_name)
                    .action(ArgAction::Set)
                    .required(true),
            );
        }

        cmd = cmd.subcommand(sub);
    }

    cmd
}

fn dispatch(matches: ArgMatches) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let (sub_name, sub_matches) = matches
        .subcommand()
        .expect("subcommand_required(true) guarantees a subcommand");

    let task = inventory::iter::<Task>
        .into_iter()
        .find(|t| t.name == sub_name)
        .expect("matched subcommand must exist in inventory");

    (task.run)(sub_matches)
}

pub fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let matches = build_cli().get_matches();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            dispatch(matches).await;
        });

    info!("task completed");
}
