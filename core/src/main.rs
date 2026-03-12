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

        for task_arg in task.args {
            use crate::task::ArgKind;

            let (action, required) = match task_arg.kind {
                ArgKind::Flag => (ArgAction::SetTrue, false),
                ArgKind::Optional => (ArgAction::Set, false),
                ArgKind::Required => (ArgAction::Set, true),
                ArgKind::Many => (ArgAction::Append, true),
            };

            sub = sub.arg(
                Arg::new(task_arg.name)
                    .long(task_arg.name)
                    .action(action)
                    .required(required),
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
