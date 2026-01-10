use std::{
    collections::HashMap, fs::{create_dir_all, read_to_string, write}, path::Path, sync::LazyLock
};

use anyhow::Context;
use serde::Deserialize;

use crate::{helpers::return_true, style::Stylesheet};

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_browser: Option<String>,
    #[serde(default)]
    pub max_library_books: Option<usize>,
    #[serde(default)]
    pub max_library_bytes: Option<u64>,
    #[serde(default = "return_true")]
    pub include_index: bool,
    #[serde(default = "return_true")]
    pub inject_navigation: bool,
    #[serde(default)]
    pub default_stylesheets: Vec<String>,
    #[serde(default, deserialize_with = "Stylesheet::deserialize_config_map")]
    pub stylesheets: HashMap<String, Stylesheet>
}

impl Default for Config {
    fn default() -> Self {
        // Can't use anyhow here because default method doesn't support result return; stability here should be guaranteed through tests instead
        static DEFAULT_CONFIG: LazyLock<Config> = LazyLock::new(|| {
            toml::from_str(Config::DEFAULT_STR)
                .expect("Internal error: failed to deserialize default config.")
        });
        DEFAULT_CONFIG.clone()
    }
}

impl Config {
    const DEFAULT_STR: &'static str = include_str!("../assets/default_config.toml");

    fn write_default(config_file_path: &Path) -> anyhow::Result<()> {
        let config_file_parent_path = config_file_path
            .parent()
            .context("Internal error: tried to write default config file to root.")?;
        match create_dir_all(&config_file_parent_path) {
            Ok(_) => match write(&config_file_path, Self::DEFAULT_STR) {
                Ok(_) => (),
                Err(_) => println!(
                    "Warning: failed to write default config file to {}.",
                    config_file_path.display()
                ),
            },
            Err(_) => println!(
                "Warning: couldn't create config file directory {}.",
                config_file_parent_path.display()
            ),
        }
        Ok(())
    }

    pub fn open(config_file_path: &Path) -> anyhow::Result<Self> {
        Ok(match read_to_string(config_file_path) {
            Ok(config_string) => match toml::from_str(&config_string) {
                Ok(config) => config,
                Err(_) => {
                    println!(
                        "Warning: config file at {} is ill-formed. Falling back on default config.",
                        config_file_path.display()
                    );
                    Self::default()
                }
            },
            Err(_) => {
                // We might want to only make the default config given a user input flag, to avoid getting it stuck in the face of future updates that might change defaults?
                println!(
                    "Couldn't read config file at {}. Attempting to create default config file.",
                    config_file_path.display()
                );
                Self::write_default(config_file_path)?;
                Self::default()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        Config::default();
    }
}
