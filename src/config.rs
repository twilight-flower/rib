use std::{
    fs::{create_dir_all, read_to_string, write},
    path::PathBuf,
};

use lazy_static::lazy_static;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_browser: Option<String>,
    #[serde(default)]
    pub max_library_books: Option<usize>,
    #[serde(default)]
    pub max_library_bytes: Option<u64>,
    // Stylesheets
}

impl Default for Config {
    fn default() -> Self {
        lazy_static! {
            static ref DEFAULT_CONFIG: Config = toml::from_str(Config::DEFAULT_STRING)
                .expect("Internal error: failed to deserialize default config.");
        }
        DEFAULT_CONFIG.clone()
    }
}

impl Config {
    const DEFAULT_STRING: &'static str = include_str!("../assets/default_config.toml");

    fn write_default(config_file_path: &PathBuf) {
        let config_file_parent_path = config_file_path
            .parent()
            .expect("Internal error: tried to write default config file to root.");
        match create_dir_all(&config_file_parent_path) {
            Ok(_) => match write(&config_file_path, Self::DEFAULT_STRING) {
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
    }

    pub fn open(config_file_path: PathBuf) -> Self {
        match read_to_string(&config_file_path) {
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
                Self::write_default(&config_file_path);
                Self::default()
            }
        }
    }
}
