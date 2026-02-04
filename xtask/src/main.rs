mod dist;

use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use clap::Parser;
use clap::Subcommand;

use crate::dist::CommandDist;

type DynError = Box<dyn std::error::Error>;

#[derive(Parser)]
struct Command {
    #[clap(subcommand)]
    sub: SubCommand,
}

impl Command {
    fn run(self) -> Result<(), DynError> {
        match self.sub {
            SubCommand::Lint(cmd) => cmd.run(),
            SubCommand::Test(cmd) => cmd.run(),
            SubCommand::Dist(cmd) => cmd.run(),
        }
    }
}

#[derive(Subcommand)]
enum SubCommand {
    #[clap(about = "Run format and clippy checks.")]
    Lint(CommandLint),
    #[clap(about = "Run unit tests.")]
    Test(CommandTest),
    #[clap(about = "Generate distributable binary package.")]
    Dist(CommandDist),
}

#[derive(Parser)]
struct CommandTest {
    #[arg(long, help = "Run tests serially and do not capture output.")]
    no_capture: bool,
}

impl CommandTest {
    fn run(self) -> Result<(), DynError> {
        run_command(make_test_cmd(self.no_capture, &[])?)
    }
}

#[derive(Parser)]
#[clap(name = "lint")]
struct CommandLint {
    #[arg(long, help = "Automatically apply lint suggestions.")]
    fix: bool,
}

impl CommandLint {
    fn run(self) -> Result<(), DynError> {
        run_command(make_clippy_cmd(self.fix)?)?;
        run_command(make_format_cmd(self.fix)?)?;
        run_command(make_taplo_cmd(self.fix)?)?;
        run_command(make_typos_cmd()?)?;

        Ok(())
    }
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn find_command(cmd: &str) -> Result<StdCommand, DynError> {
    let exe = which::which(cmd)?;

    let mut command = StdCommand::new(exe);
    command.current_dir(project_root());
    Ok(command)
}

fn ensure_installed(bin: &str, crate_name: &str) -> Result<(), DynError> {
    if which::which(bin).is_err() {
        let mut cmd = find_command("cargo")?;
        cmd.args(["install", crate_name]);
        run_command(cmd)?;
    }
    Ok(())
}

fn run_command(mut cmd: StdCommand) -> Result<(), DynError> {
    println!("{cmd:?}");
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed: {status}").into())
    }
}

fn make_test_cmd(no_capture: bool, features: &[&str]) -> Result<StdCommand, DynError> {
    let mut cmd = find_command("cargo")?;
    cmd.args(["test", "--workspace", "--no-default-features"]);
    if !features.is_empty() {
        cmd.args(["--features", features.join(",").as_str()]);
    }
    if no_capture {
        cmd.args(["--", "--nocapture"]);
    }
    Ok(cmd)
}

fn make_format_cmd(fix: bool) -> Result<StdCommand, DynError> {
    let mut cmd = find_command("cargo")?;
    cmd.args(["+nightly", "fmt", "--all"]);
    if !fix {
        cmd.arg("--check");
    }
    Ok(cmd)
}

fn make_clippy_cmd(fix: bool) -> Result<StdCommand, DynError> {
    let mut cmd = find_command("cargo")?;
    cmd.args([
        "+nightly",
        "clippy",
        "--tests",
        "--all-features",
        "--all-targets",
        "--workspace",
    ]);
    if fix {
        cmd.args(["--allow-staged", "--allow-dirty", "--fix"]);
    } else {
        cmd.args(["--", "-D", "warnings"]);
    }
    Ok(cmd)
}

fn make_typos_cmd() -> Result<StdCommand, DynError> {
    ensure_installed("typos", "typos-cli")?;
    find_command("typos")
}

fn make_taplo_cmd(fix: bool) -> Result<StdCommand, DynError> {
    ensure_installed("taplo", "taplo-cli")?;
    let mut cmd = find_command("taplo")?;
    if fix {
        cmd.args(["format"]);
    } else {
        cmd.args(["format", "--check"]);
    }
    Ok(cmd)
}

fn main() {
    let cmd = Command::parse();
    if let Err(e) = cmd.run() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}
