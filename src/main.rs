mod library;

use std::path::PathBuf;

use argh::FromArgs;
use directories::ProjectDirs;
use epub::doc::EpubDoc;

use crate::library::{Library, LibraryBookInfo};

//////////////
//   Args   //
//////////////

#[derive(Clone, Copy, Debug, FromArgs)]
#[argh(subcommand)]
enum Subcommand {} // Placeholder

#[derive(Clone, Debug, FromArgs)]
/// Minimalist EPUB reader.
struct Args {
    #[argh(subcommand)]
    subcommand: Option<Subcommand>,
    #[argh(positional)]
    /// epub path to open
    path: PathBuf, // Maybe switch to vec to allow opening multiple epubs at once?
}

//////////////
//   Main   //
//////////////

fn main() {
    let args: Args = argh::from_env();

    let project_dirs = ProjectDirs::from("", "", "rib")
        .expect("Couldn't open cache: no home directory path found.");

    let library_path = project_dirs.data_local_dir().join("library");
    let library = Library::open(library_path);

    // Once we've got support for multiple formats, do branching here.
    let book = EpubDoc::new(&args.path).expect(&format!("Couldn't open {} as EPUB.", args.path.display()));

    // EPUB-handling may work better in its own module rather than in main; but keep it here for now for simplicity
    let book_info = LibraryBookInfo::from_epub(&library, &book);
}
