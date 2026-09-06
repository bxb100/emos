use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

use crate::DynError;
use crate::find_command;
use crate::workspace_root;

#[derive(Parser)]
pub(crate) struct Dist {
    #[arg(long = "package", help = "Binary name to distribute")]
    binary: String,
    #[arg(long, help = "Strip the binary to reduce size")]
    strip: Option<bool>,
}

#[inline]
fn dist_dir() -> PathBuf {
    workspace_root().join("target/dist")
}

impl Dist {
    pub(crate) fn run(&self) -> Result<(), DynError> {
        let _ = fs::remove_dir_all(dist_dir());
        fs::create_dir_all(dist_dir())?;

        self.dist_binary()
    }

    /// Copy from https://github.com/matklad/cargo-xtask/blob/master/examples/hello-world/xtask/src/main.rs
    fn dist_binary(&self) -> Result<(), DynError> {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(workspace_root())
            .args(["build", "--release"])
            .status()?;

        if !status.success() {
            Err("cargo build failed")?;
        }

        let dst = workspace_root().join(format!("target/release/{}", self.binary));

        fs::copy(&dst, dist_dir().join(&self.binary))?;

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
