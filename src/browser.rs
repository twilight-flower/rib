use std::{path::Path, process::Command};

use anyhow::{Context, bail};

pub fn open(path: &Path, browser: &Option<String>) -> anyhow::Result<()> {
    let launch_command = match browser {
        Some(browser_command) => browser_command,
        None => {
            if cfg!(windows) {
                "explorer"
            } else if cfg!(target_os = "linux") {
                "xdg-open"
            } else if cfg!(target_os = "macos") {
                // TODO: test this and make sure it works right
                "open"
            } else {
                bail!(
                    "Unable to open {} in default browser: unsupported OS.",
                    path.display()
                )
            }
        }
    };

    Command::new(launch_command)
        .arg(path)
        .spawn()
        .with_context(|| format!("Failed to open {} in browser.", path.display()))?;
    Ok(())
}
