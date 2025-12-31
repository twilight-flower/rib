mod book;
mod browser;
mod config;
mod helpers;
mod library;

use std::path::{Path, PathBuf};

use argh::{ArgsInfo, FromArgs};
use directories::ProjectDirs;

use crate::{book::open_books, config::Config, library::Library};

//////////////
//   Args   //
//////////////

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// clear books from library
#[argh(subcommand, name = "clear")]
struct LibraryClearArgs {
    #[argh(switch, short = 'a')]
    /// clear all books from library
    all: bool,
    #[argh(option, short = 'b')]
    /// clear books until no more than this many remain in the library
    max_books: Option<usize>,
    #[argh(option, short = 'B')]
    /// clear books until library size is no more than this many bytes
    max_bytes: Option<u64>,
    #[argh(positional)]
    /// clear books with these ids
    ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// list books in library
#[argh(subcommand, name = "list")]
struct LibraryListArgs {}

#[derive(Clone, Debug, FromArgs, ArgsInfo)]
#[argh(subcommand)]
enum LibrarySubcommand {
    Clear(LibraryClearArgs),
    List(LibraryListArgs),
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
    Library(LibraryArgs),
} // Placeholder

// When updating to support non-EPUB input, adjust docstrings here accordingly
#[derive(Clone, Debug, FromArgs, ArgsInfo)]
/// Minimalist EPUB reader.
struct Args {
    #[argh(subcommand)]
    subcommand: Option<ArgsSubcommand>,
    #[argh(option, short = 'b')]
    /// browser to open book with (default: system default browser)
    browser: Option<String>,
    #[argh(positional)]
    /// epub paths to open
    paths: Vec<PathBuf>,
}

//////////////
//   Main   //
//////////////

fn main() {
    let args: Args = argh::from_env();

    let project_dirs = ProjectDirs::from("", "", "rib")
        .expect("Couldn't open library: no home directory path found.");

    let config_path = project_dirs.config_dir().join("config.toml");
    let config = Config::open(config_path);

    let library_path = project_dirs.data_local_dir().join("library");
    let mut library = Library::open(library_path);

    match args.subcommand {
        Some(subcommand) => match subcommand {
            ArgsSubcommand::Library(library_args) => match library_args.subcommand {
                LibrarySubcommand::Clear(library_clear_args) => match library_clear_args.all {
                    true => library.clear(Some(0), None, &[]),
                    false => library.clear(
                        library_clear_args.max_books,
                        library_clear_args.max_bytes,
                        &library_clear_args.ids,
                    ),
                },
                LibrarySubcommand::List(_) => println!("Placeholder: library list subcommand."),
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
                    .expect_err("Internal error: failed to print help text.");
                println!("{}", help_text.output);
            }
            _ => {
                let browser = match (args.browser, config.default_browser) {
                    (Some(args_browser), _) => Some(args_browser),
                    (None, Some(config_browser)) => Some(config_browser),
                    (None, None) => None,
                };
                open_books(
                    &mut library,
                    args.paths,
                    browser,
                    config.max_library_books,
                    config.max_library_bytes,
                )
            }
        },
    }
}
