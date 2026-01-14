mod book;
mod browser;
mod cli;
mod config;
mod css;
mod epub;
mod helpers;
mod library;
mod style;

use anyhow::Context;
use clap::Parser;
use directories::ProjectDirs;

use crate::{
    book::open_books,
    cli::{Cli, CliSubcommand, ConfigSubcommand, LibrarySubcommand},
    config::Config,
    library::Library,
    style::Style,
};

//////////////
//   Main   //
//////////////

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let project_dirs = ProjectDirs::from("", "", "rib")
        .context("Couldn't open library: no home directory path found.")?;

    let config_path = project_dirs.config_dir().join("config.toml");
    let config = Config::open(&config_path)?;

    let library_path = project_dirs.data_local_dir().join("library");
    let mut library = Library::open(library_path.clone())?;

    match args.subcommand {
        Some(subcommand) => match subcommand {
            CliSubcommand::Config(config_subcommand) => match config_subcommand {
                ConfigSubcommand::Path => {
                    println!("{}", config_path.display());
                    Ok(())
                }
            },
            CliSubcommand::Library(library_subcommand) => match library_subcommand {
                LibrarySubcommand::Clear {
                    ids,
                    max_books,
                    max_bytes,
                    all,
                } => match all {
                    true => library.clear(Some(0), None, &[]),
                    false => library.clear(max_books, max_bytes, &ids),
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
                        Some(sheets) => sheets
                            .iter()
                            .map(|sheet_name| Style {
                                include_index,
                                inject_navigation,
                                stylesheet: config.get_stylesheet(sheet_name, &args.styles),
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
