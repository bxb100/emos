mod dist;

use std::path::Path;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use clap::Parser;
use clap::Subcommand;

use crate::dist::Dist;

type DynError = Box<dyn std::error::Error>;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    fn run(self) -> Result<(), DynError> {
        match self.command {
            Command::Lint(cmd) => cmd.run(),
            Command::Test(cmd) => cmd.run(),
            Command::Dist(cmd) => cmd.run(),
        }
    }
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run format and clippy checks.")]
    Lint(Lint),
    #[command(about = "Run unit tests.")]
    Test(Test),
    #[command(about = "Generate distributable binary package.")]
    Dist(Dist),
}

#[derive(Parser)]
struct Test {
    #[arg(long, help = "Run tests serially and do not capture output.")]
    no_capture: bool,
}

impl Test {
    fn run(self) -> Result<(), DynError> {
        run_command(test_command(self.no_capture, &[])?)
    }
}

#[derive(Parser)]
#[command(name = "lint")]
struct Lint {
    #[arg(long, help = "Automatically apply lint suggestions.")]
    fix: bool,
}

impl Lint {
    fn run(self) -> Result<(), DynError> {
        run_command(clippy_command(self.fix)?)?;
        run_command(format_command(self.fix)?)?;
        run_command(taplo_command(self.fix)?)?;
        run_command(typos_command()?)?;

        Ok(())
    }
}

fn workspace_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn find_command(cmd: &str) -> Result<ProcessCommand, DynError> {
    let exe = which::which(cmd)?;

    let mut command = ProcessCommand::new(exe);
    command.current_dir(workspace_root());
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

fn run_command(mut cmd: ProcessCommand) -> Result<(), DynError> {
    println!("{cmd:?}");
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed: {status}").into())
    }
}

fn test_command(no_capture: bool, features: &[&str]) -> Result<ProcessCommand, DynError> {
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

fn format_command(fix: bool) -> Result<ProcessCommand, DynError> {
    let mut cmd = find_command("cargo")?;
    cmd.args(["+nightly", "fmt", "--all"]);
    if !fix {
        cmd.arg("--check");
    }
    Ok(cmd)
}

fn clippy_command(fix: bool) -> Result<ProcessCommand, DynError> {
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

fn typos_command() -> Result<ProcessCommand, DynError> {
    ensure_installed("typos", "typos-cli")?;
    find_command("typos")
}

fn taplo_command(fix: bool) -> Result<ProcessCommand, DynError> {
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
    let cmd = Cli::parse();
    if let Err(e) = cmd.run() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}
