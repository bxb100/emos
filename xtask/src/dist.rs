use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

use crate::DynError;
use crate::find_command;
use crate::project_root;

#[derive(Parser)]
pub(crate) struct CommandDist {
    #[arg(long, help = "Binary package name to dist")]
    package: String,
    #[arg(long, help = "Strip the binary to reduce size")]
    strip: Option<bool>,
}

#[inline]
fn dist_dir() -> PathBuf {
    project_root().join("target/dist")
}

impl CommandDist {
    pub(crate) fn run(&self) -> Result<(), DynError> {
        let _ = fs::remove_dir_all(dist_dir());
        fs::create_dir_all(dist_dir())?;

        self.dist_binary()
    }

    /// Copy from https://github.com/matklad/cargo-xtask/blob/master/examples/hello-world/xtask/src/main.rs
    fn dist_binary(&self) -> Result<(), DynError> {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(project_root())
            .args(["build", "--release"])
            .status()?;

        if !status.success() {
            Err("cargo build failed")?;
        }

        let dst = project_root().join(format!("target/release/{}", self.package));

        fs::copy(&dst, dist_dir().join(&self.package))?;

        if let Some(strip) = self.strip
            && strip
        {
            if find_command("strip").is_ok() {
                eprintln!("stripping the binary");
                let status = Command::new("strip").arg(&dst).status()?;
                if !status.success() {
                    Err("strip failed")?;
                }
            } else {
                eprintln!("no `strip` utility found")
            }
        }

        Ok(())
    }
}
