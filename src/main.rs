use std::path::PathBuf;

use argh::FromArgs;

//////////////
//   Args   //
//////////////

#[derive(FromArgs)]
#[argh(subcommand)]
enum Subcommand {} // Placeholder

#[derive(FromArgs)]
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
    println!("Attempting to open {}.", args.path.display());
}
