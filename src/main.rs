mod book;
mod browser;
mod helpers;
mod library;

use std::path::{Path, PathBuf};

use argh::{ArgsInfo, FromArgs};
use directories::ProjectDirs;

use crate::{book::open_books, library::Library};

//////////////
//   Args   //
//////////////

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// list books in library
#[argh(subcommand, name = "list")]
struct LibraryListArgs {}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// clear books from library
#[argh(subcommand, name = "clear")]
struct LibraryClearArgs {}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
#[argh(subcommand)]
enum LibrarySubcommand {
    List(LibraryListArgs),
    Clear(LibraryClearArgs),
}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
/// interact with rib's library of previously-opened books
#[argh(subcommand, name = "library")]
struct LibraryArgs {
    #[argh(subcommand)]
    subcommand: LibrarySubcommand,
}

#[derive(Clone, Copy, Debug, FromArgs, ArgsInfo)]
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
        .expect("Couldn't open cache: no home directory path found.");

    let library_path = project_dirs.data_local_dir().join("library");
    let mut library = Library::open(library_path);

    match args.subcommand {
        Some(subcommand) => match subcommand {
            ArgsSubcommand::Library(library_args) => match library_args.subcommand {
                LibrarySubcommand::Clear(_) => println!("Placeholder: library clear subcommand."),
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
            _ => open_books(&mut library, args.paths),
        },
    }
}
