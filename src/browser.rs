use std::{path::PathBuf, process::Command};

pub fn open(path: &PathBuf) {
    // TODO: add support for Mac
    let launch_command = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "linux") {
        "xdg-open"
    } else {
        panic!(
            "Unable to open {} in browser: unsupported OS.",
            path.display()
        )
    };

    Command::new(launch_command)
        .arg(&path)
        .output()
        .expect(&format!("Failed to open {} in browser.", path.display()));
}
