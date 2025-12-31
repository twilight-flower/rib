use std::{path::PathBuf, process::Command};

pub fn open(path: &PathBuf, browser: &Option<String>) {
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
                panic!(
                    "Unable to open {} in default browser: unsupported OS.",
                    path.display()
                )
            }
        }
    };

    Command::new(launch_command)
        .arg(path)
        .spawn()
        .expect(&format!("Failed to open {} in browser.", path.display()));
}
