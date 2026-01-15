use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::Context;
use epub::doc::EpubDoc;

use crate::{library::Library, style::Style};

pub struct StylesForBookOpen {
    pub styles: Vec<Style>,
    pub using_default_style: bool,
}

fn open_epub(
    library: &mut Library,
    path: &Path,
    request_time: SystemTime,
    browser: &Option<String>,
    styles: &StylesForBookOpen,
) -> anyhow::Result<String> {
    // Returns id of opened EPUB

    let mut epub =
        EpubDoc::new(path).with_context(|| format!("Couldn't open {} as EPUB.", path.display()))?;
    let id = library.register_epub_and_get_id(&mut epub, request_time)?;

    let styles_to_open = match styles.using_default_style {
        true => {
            let last_opened_styles = library.get_last_opened_styles(&id)?;
            match last_opened_styles.is_empty() {
                true => styles.styles.clone(),
                false => last_opened_styles.clone(),
            }
        }
        false => styles.styles.clone(),
    };
    library.register_book_styles(&id, &styles_to_open)?;

    for style in styles_to_open {
        library.open_book(&id, request_time, browser, &style)?;
    }

    Ok(id)
}

pub fn open_books(
    library: &mut Library,
    paths: Vec<PathBuf>,
    browser: Option<String>,
    styles: StylesForBookOpen,
    max_books: Option<usize>,
    max_bytes: Option<u64>,
) -> anyhow::Result<()> {
    let mut opened_book_ids = HashSet::new();

    let request_time = SystemTime::now();
    for path in paths {
        // Once we've got support for multiple formats, do branching here and maybe factor EPUB-handling into its own module.
        let book_id = open_epub(library, &path, request_time, &browser, &styles)?;
        opened_book_ids.insert(book_id);
    }

    library.truncate(max_books, max_bytes, &opened_book_ids)?;
    Ok(())
}
