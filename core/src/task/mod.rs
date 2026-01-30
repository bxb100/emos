use std::pin::Pin;

use clap::ArgMatches;

pub(crate) mod sync_video_list;
pub(crate) mod watch_basic_genre;

pub type TaskFn = fn(&ArgMatches) -> Pin<Box<dyn Future<Output = ()> + Send>>;

pub(crate) struct Task {
    pub(crate) name: &'static str,
    pub(crate) args: &'static [&'static str],
    pub(crate) run: TaskFn,
}
#[macro_export]
macro_rules! add_task {
    (
        $name:literal,
        $fun:ident $(,)?
        $($var:ident : $ty:ty = $arg_name:literal ),*
    ) => {
        inventory::submit! {
            #[allow(unused)]
            Task {
                name: $name,
                args: {
                    const ARGS: &[&str] = &[$($arg_name),*];
                    ARGS
                },
                run: |arg: &ArgMatches| {
                    $(
                        let $var: $ty = arg
                            .get_one::<$ty>($arg_name)
                            .expect("missing required argument")
                            .to_owned();
                    )*
                    Box::pin(async move {
                        $fun($($var),*).await.context($name).unwrap();
                    })
                }
            }
        }
    };
}

inventory::collect!(Task);
