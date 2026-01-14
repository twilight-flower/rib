mod book;
mod browser;
mod config;
mod css;
mod epub;
mod helpers;
mod library;
mod style;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;

use crate::{book::open_books, config::Config, library::Library, style::Style};

#[derive(Clone, Copy, Debug, Subcommand)]
enum ConfigSubcommand {
    /// print path to config file
    Path,
}

#[derive(Clone, Debug, Subcommand)]
enum LibrarySubcommand {
    /// clear books from library
    Clear {
        // TODO: figure out how to give this a help subcommand
        /// ids of books to clear even if they're otherwise within any specified library size limits
        ids: Vec<String>,
        /// clear all books from library
        #[arg(short, long)]
        all: bool,
        /// integer; clear books until no more than this many remain in the library
        #[arg(short = 'b', long)]
        max_books: Option<usize>,
        /// integer; clear books until library size is no more than this many bytes
        #[arg(short = 'B', long)]
        max_bytes: Option<u64>,
    },
    /// list books in library
    // TODO: add option for a more machine-readable output, and maybe store more metadata in the library so the machines and/or nonmachines can filter on more information
    List,
    /// print path to library directory
    Path,
}

#[derive(Clone, Debug, Subcommand)]
enum ArgsSubcommand {
    /// interact with rib's configuration
    #[command(subcommand)]
    Config(ConfigSubcommand),
    /// interact with rib's library of previously-opened books
    #[command(subcommand)]
    Library(LibrarySubcommand),
}

#[derive(Clone, Debug, Parser)]
#[command(version, about)]
#[clap(arg_required_else_help(true))]
struct Args {
    #[command(subcommand)]
    subcommand: Option<ArgsSubcommand>,
    /// epub paths to open
    paths: Vec<PathBuf>,
    /// command to open book with
    #[arg(short, long)]
    browser: Option<String>,
    /// include index when opening book
    #[arg(short = 'i', long)]
    include_index: Option<bool>,
    /// inject navigation when opening book
    #[arg(short = 'n', long)]
    inject_navigation: Option<bool>,
    /// stylesheet(s), by name as defined in config, to open book with
    #[arg(short = 'S', long)]
    stylesheets: Vec<String>,
    /// open raw book without index or navigation or stylesheets
    #[arg(short, long)]
    raw: bool,
}

//////////////
//   Main   //
//////////////

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let project_dirs = ProjectDirs::from("", "", "rib")
        .context("Couldn't open library: no home directory path found.")?;

    let config_path = project_dirs.config_dir().join("config.toml");
    let config = Config::open(&config_path)?;

    let library_path = project_dirs.data_local_dir().join("library");
    let mut library = Library::open(library_path.clone())?;

    match args.subcommand {
        Some(subcommand) => match subcommand {
            ArgsSubcommand::Config(config_subcommand) => match config_subcommand {
                ConfigSubcommand::Path => {
                    println!("{}", config_path.display());
                    Ok(())
                }
            },
            ArgsSubcommand::Library(library_subcommand) => match library_subcommand {
                LibrarySubcommand::Clear {ids, max_books, max_bytes, all} => match all {
                    true => library.clear(Some(0), None, &[]),
                    false => library.clear(
                        max_books,
                        max_bytes,
                        &ids,
                    ),
                },
                LibrarySubcommand::List => library.list(),
                LibrarySubcommand::Path => {
                    println!("{}", library_path.display());
                    Ok(())
                }
            },
        },
        None => {
            let include_index = match (args.include_index, config.include_index) {
                (Some(arg_value), _) => arg_value,
                (_, config_value) => config_value,
            };
            let inject_navigation = match (args.inject_navigation, config.inject_navigation) {
                (Some(arg_value), _) => arg_value,
                (_, config_value) => config_value,
            };
            let styles = match args.raw {
                true => vec![Style::raw()],
                false => {
                    let stylesheets = match (
                        args.stylesheets.is_empty(),
                        config.default_stylesheets.is_empty(),
                    ) {
                        (true, true) => None,
                        (true, false) => Some(&config.default_stylesheets),
                        (false, _) => Some(&args.stylesheets),
                    };
                    match stylesheets {
                        None => vec![Style {
                            include_index,
                            inject_navigation,
                            stylesheet: None,
                        }],
                        Some(stylesheets) => stylesheets
                            .iter()
                            .map(|stylesheet_name| Style {
                                include_index,
                                inject_navigation,
                                stylesheet: config.get_stylesheet(stylesheet_name),
                            })
                            .collect(),
                    }
                }
            };
            let open_all_styles = match args.stylesheets.is_empty() {
                true => false,
                false => true,
            };
            let browser = match (args.browser, config.default_browser) {
                (Some(args_browser), _) => Some(args_browser),
                (None, Some(config_browser)) => Some(config_browser),
                (None, None) => None,
            };
            open_books(
                &mut library,
                args.paths,
                browser,
                styles,
                open_all_styles,
                config.max_library_books,
                config.max_library_bytes,
            )
        }
    }
}
