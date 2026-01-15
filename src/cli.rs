use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

// #[derive(Clone, Debug, FromArgs)]
// /// set book text color
// #[argh(subcommand, name = "text_color")]
// struct ArgsStyleTextColorArgs {
//     #[argh(positional)]
//     /// color attribute value for book text
//     style: Option<String>,
// }

// #[derive(Clone, Debug, FromArgs)]
// #[argh(subcommand)]
// enum ArgsStyleSubcommand {
//     TextColor(ArgsStyleTextColorArgs),
// }

// #[derive(Clone, Debug, FromArgs)]
// /// Minimalist EPUB reader.
// struct Args {
//     /// individual style(s) to override values of
//     style: Vec<ArgsStyleSubcommand>,
// }

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum ConfigSubcommand {
    /// print path to config file
    Path,
}

#[derive(Clone, Debug, Subcommand)]
pub enum LibrarySubcommand {
    /// clear books from library
    #[group(required = true, multiple = true)]
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
pub enum CliSubcommand {
    /// interact with rib's configuration
    #[command(subcommand)]
    Config(ConfigSubcommand),
    /// interact with rib's library of previously-opened books
    #[command(subcommand)]
    Library(LibrarySubcommand),
}

#[derive(Clone, Debug, Args)]
pub struct CliStyleCommands {
    // It'd be nice to replace this with something more subcommandlike. But that doesn't seem to be a feature clap currently offers; it wants only one subcommand per command-layer.
    /// color attribute value for book body
    #[arg(long)]
    pub text_color: Option<String>,
    /// color attribute value for book links
    #[arg(long)]
    pub link_color: Option<String>,
    /// background-color attribute value for book body
    #[arg(long)]
    pub background_color: Option<String>,
    /// margin-left and margin-right attribute values for book body
    #[arg(long)]
    pub margin_size: Option<String>,
    /// max-height attribute value for book img embeds
    #[arg(long)]
    pub max_image_height: Option<String>,
    /// max-width attribute value for book img embeds
    #[arg(long)]
    pub max_image_width: Option<String>,

    /// user-supplied text color overrides book's internally-specified text color
    #[arg(long)]
    pub text_color_override: Option<bool>,
    /// user-supplied link color overrides book's internally-specified link color
    #[arg(long)]
    pub link_color_override: Option<bool>,
    /// user-supplied background color overrides book's internally-specified background color
    #[arg(long)]
    pub background_color_override: Option<bool>,
    /// user-supplied margin size overrides book's internally-specified margin sizes
    #[arg(long)]
    pub margin_size_override: Option<bool>,
    /// user-supplied max image height overrides book's internally-specified max image height
    #[arg(long)]
    pub max_image_height_override: Option<bool>,
    /// user-supplied max image width overrides book's internally-specified max image width
    #[arg(long)]
    pub max_image_width_override: Option<bool>,
}

impl CliStyleCommands {
    pub fn is_undefined(&self) -> bool {
        self.text_color.is_none()
            && self.link_color.is_none()
            && self.background_color.is_none()
            && self.margin_size.is_none()
            && self.max_image_height.is_none()
            && self.max_image_width.is_none()
            && self.text_color_override.is_none()
            && self.link_color_override.is_none()
            && self.background_color_override.is_none()
            && self.margin_size_override.is_none()
            && self.max_image_height_override.is_none()
            && self.max_image_width_override.is_none()
    }
}

#[derive(Clone, Debug, Parser)]
#[command(
    version,
    about,
    arg_required_else_help = true,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<CliSubcommand>,
    /// epub paths to open
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// command to open book with
    #[arg(short, long)]
    pub browser: Option<String>,
    /// include index when opening book
    #[arg(short = 'i', long)]
    pub include_index: Option<bool>,
    /// inject navigation when opening book
    #[arg(short = 'n', long)]
    pub inject_navigation: Option<bool>,
    /// stylesheet(s), by name as defined in config, to open book with
    #[arg(short = 'S', long)]
    pub stylesheets: Vec<String>,
    /// individual style(s) to set, overriding any values specified in stylesheet(s)
    // (The docstring here is currently ignored due to the flattened layout)
    #[command(flatten)]
    pub styles: CliStyleCommands,
    /// open raw book without adding index or navigation or styling
    #[arg(short, long)]
    pub raw: bool,
    // TODO: 'verbose' arg to print non-warning info messages like the dump path descriptions
    // TODO: consider whether to refactor opening-books-by-path into a subcommand and then present it here only as a flattened default-if-no-other-subcommand
}
