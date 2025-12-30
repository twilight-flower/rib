use std::path::PathBuf;

use epub::doc::EpubDoc;

use crate::{Args, library::Library};

fn open_epub(library: &mut Library, path: &PathBuf, browser: &Option<String>) {
    // Once we've got support for multiple formats, do branching here and factor EPUB-handling into its own module.
    let mut epub = EpubDoc::new(path).expect(&format!("Couldn't open {} as EPUB.", path.display()));
    let id = library.register_epub_and_get_id(&mut epub, path);

    // TODO: process book into non-raw format, and open that instead, if needed

    library.open_raw(&id, browser);
}

pub fn open_books(library: &mut Library, args: Args) {
    // Once we're doing library-size-limiting this is going to need to get fancier to make sure the cache-update doesn't trim any of the newly-opened books
    for path in args.paths {
        open_epub(library, &path, &args.browser);
    }
}
