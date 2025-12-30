use std::path::PathBuf;

use epub::doc::EpubDoc;

use crate::library::Library;

fn open_epub(library: &mut Library, path: PathBuf) {
    // Once we've got support for multiple formats, do branching here and factor EPUB-handling into its own module.
    let mut epub =
        EpubDoc::new(&path).expect(&format!("Couldn't open {} as EPUB.", path.display()));
    let id = library.register_epub_and_get_id(&mut epub, &path);

    // TODO: process book into non-raw format, and open that instead, if needed

    library.open_raw(&id);
}

pub fn open_books(library: &mut Library, paths: Vec<PathBuf>) {
    // Once we're doing library-size-limiting this is going to need to get fancier to make sure the cache-update doesn't trim any of the newly-opened books
    for path in paths {
        open_epub(library, path);
    }
}
