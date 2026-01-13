use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::Context;
use epub::doc::EpubDoc;

use crate::{library::Library, style::Style};

fn open_epub(
    library: &mut Library,
    path: &Path,
    request_time: SystemTime,
    browser: &Option<String>,
    styles: &[Style],
    open_all_styles: bool,
) -> anyhow::Result<String> {
    // Returns id of opened EPUB

    let mut epub =
        EpubDoc::new(path).with_context(|| format!("Couldn't open {} as EPUB.", path.display()))?;
    let id = library.register_epub_and_get_id(&mut epub, path, request_time)?;

    library.register_book_styles(&id, styles)?;

    match open_all_styles {
        true => {
            for style in styles {
                library.open_book(&id, request_time, browser, style)?;
            }
        }
        false => {
            let first_style_specified = styles
                .first()
                .context("Internal error: no target style defined.")?;
            library.open_book(&id, request_time, browser, first_style_specified)?;
        }
    }

    Ok(id)
}

pub fn open_books(
    library: &mut Library,
    paths: Vec<PathBuf>,
    browser: Option<String>,
    styles: Vec<Style>,
    open_all_styles: bool,
    max_books: Option<usize>,
    max_bytes: Option<u64>,
) -> anyhow::Result<()> {
    let mut opened_book_ids = HashSet::new();

    let request_time = SystemTime::now();
    for path in paths {
        // Once we've got support for multiple formats, do branching here and maybe factor EPUB-handling into its own module.
        let book_id = open_epub(
            library,
            &path,
            request_time,
            &browser,
            &styles,
            open_all_styles,
        )?;
        opened_book_ids.insert(book_id);
    }

    library.truncate(max_books, max_bytes, &opened_book_ids)?;
    Ok(())
}
