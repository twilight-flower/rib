use std::{collections::HashSet, path::PathBuf};

use chrono::{DateTime, Utc};
use epub::doc::EpubDoc;

use crate::library::Library;

fn open_epub(
    library: &mut Library,
    path: &PathBuf,
    request_time: DateTime<Utc>,
    browser: &Option<String>,
) -> String {
    // Returns id of opened EPUB

    let mut epub = EpubDoc::new(path).expect(&format!("Couldn't open {} as EPUB.", path.display()));
    let id = library.register_epub_and_get_id(&mut epub, path, request_time);

    // TODO: process book into non-raw format, and open that instead, if needed

    library.open_book_raw(&id, request_time, browser);

    id
}

pub fn open_books(
    library: &mut Library,
    paths: Vec<PathBuf>,
    browser: Option<String>,
    max_books: Option<usize>,
    max_bytes: Option<u64>,
) {
    let mut opened_book_ids = HashSet::new();

    let request_time = Utc::now();
    for path in paths {
        // Once we've got support for multiple formats, do branching here and maybe factor EPUB-handling into its own module.
        let book_id = open_epub(library, &path, request_time, &browser);
        opened_book_ids.insert(book_id);
    }

    library.truncate(max_books, max_bytes, &opened_book_ids);
}
