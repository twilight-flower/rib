mod book;
mod browser;
mod config;
mod epub;
mod helpers;
mod library;
mod style;

use std::path::{Path, PathBuf};

use anyhow::Context;
use argh::{ArgsInfo, FromArgs};
use directories::ProjectDirs;

use crate::{book::open_books, config::Config, library::Library, style::Style};

//////////////
//   Args   //
//////////////

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// print path to config file
#[argh(subcommand, name = "path")]
struct ConfigPathArgs {}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
#[argh(subcommand)]
enum ConfigSubcommand {
    Path(ConfigPathArgs),
}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// interact with rib's configuration
#[argh(subcommand, name = "config")]
struct ConfigArgs {
    #[argh(subcommand)]
    subcommand: ConfigSubcommand,
}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// clear books from library
#[argh(subcommand, name = "clear")]
struct LibraryClearArgs {
    #[argh(switch, short = 'a')]
    /// clear all books from library
    all: bool,
    #[argh(option, short = 'b')]
    /// integer; clear books until no more than this many remain in the library
    max_books: Option<usize>,
    #[argh(option, short = 'B')]
    /// integer; clear books until library size is no more than this many bytes
    max_bytes: Option<u64>,
    #[argh(positional)]
    /// ids of books to clear even if they're otherwise within any specified library size limits
    ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// list books in library
#[argh(subcommand, name = "list")]
struct LibraryListArgs {
    // Empty for now; later on, add option for a more machine-readable output, and maybe store more metadata in the library so the machines and/or nonmachines can filter on more information
}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// print path to library directory
#[argh(subcommand, name = "path")]
struct LibraryPathArgs {}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
#[argh(subcommand)]
enum LibrarySubcommand {
    Clear(LibraryClearArgs),
    List(LibraryListArgs),
    Path(LibraryPathArgs),
}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// interact with rib's library of previously-opened books
#[argh(subcommand, name = "library")]
struct LibraryArgs {
    #[argh(subcommand)]
    subcommand: LibrarySubcommand,
}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
#[argh(subcommand)]
enum ArgsSubcommand {
    Config(ConfigArgs),
    Library(LibraryArgs),
} // Placeholder

// When updating to support non-EPUB input, adjust docstrings here accordingly
#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// Minimalist EPUB reader.
struct Args {
    #[argh(subcommand)]
    subcommand: Option<ArgsSubcommand>,
    #[argh(positional)]
    /// epub paths to open
    paths: Vec<PathBuf>,
    #[argh(option, short = 'b')]
    /// command to open book with (default: system default web browser)
    browser: Option<String>,
    // Plausibly add options to control index and navigation inclusion/exclusion individually here, in addition to or instead of the 'raw' shorthand and the config file
    #[argh(switch, short = 'r')]
    /// open raw book without index or navigation or stylesheets
    raw: bool,
}

//////////////
//   Main   //
//////////////

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let project_dirs = ProjectDirs::from("", "", "rib")
        .context("Couldn't open library: no home directory path found.")?;

    let config_path = project_dirs.config_dir().join("config.toml");
    let config = Config::open(&config_path)?;

    let library_path = project_dirs.data_local_dir().join("library");
    let mut library = Library::open(library_path.clone())?;

    match args.subcommand {
        Some(subcommand) => match subcommand {
            ArgsSubcommand::Config(config_args) => match config_args.subcommand {
                ConfigSubcommand::Path(_) => Ok(println!("{}", config_path.display())),
            },
            ArgsSubcommand::Library(library_args) => match library_args.subcommand {
                LibrarySubcommand::Clear(library_clear_args) => match library_clear_args.all {
                    true => library.clear(Some(0), None, &[]),
                    false => library.clear(
                        library_clear_args.max_books,
                        library_clear_args.max_bytes,
                        &library_clear_args.ids,
                    ),
                },
                LibrarySubcommand::List(_) => library.list(),
                LibrarySubcommand::Path(_) => Ok(println!("{}", library_path.display())),
            },
        },
        None => match args.paths.len() {
            0 => {
                // Print argh's help text and exit
                let first_arg = std::env::args().next();
                let run_command = first_arg
                    .as_ref()
                    .and_then(|command_str| {
                        Path::new(command_str)
                            .file_name()
                            .and_then(|executable_name| executable_name.to_str())
                    })
                    .unwrap_or("rib");
                let help_text = Args::from_args(&[run_command], &["help"])
                    .expect_err("Internal error: failed to print help text."); // Error type here isn't anyhow-compatible
                println!("{}", help_text.output);
                Ok(())
            }
            _ => {
                let browser = match (args.browser, config.default_browser) {
                    (Some(args_browser), _) => Some(args_browser),
                    (None, Some(config_browser)) => Some(config_browser),
                    (None, None) => None,
                };
                let styles = match args.raw {
                    // Once we want user-specified styling support we'll need more here. Make sure the vec is always nonempty: if the user runs the specify-style flag and then specifies empty-set-of-styles, use default as if it's unspecified
                    true => vec![Style::raw()],
                    false => vec![
                        Style::default()
                            .include_index(config.include_index)
                            .inject_navigation(config.inject_navigation),
                    ],
                };
                open_books(
                    &mut library,
                    args.paths,
                    browser,
                    styles,
                    config.max_library_books,
                    config.max_library_bytes,
                )
            }
        },
    }
}
