use std::{
    fs::{create_dir_all, write, File},
    io::BufReader,
    path::PathBuf,
};

use epub::doc::EpubDoc;

use crate::library::{Library, LibraryBookInfo};

fn dump_raw_book(
    library: &Library,
    book: &mut EpubDoc<BufReader<File>>,
    book_info: &LibraryBookInfo,
    book_path: &PathBuf,
) {
    let book_dump_dir = library
        .library_path
        .join(&book_info.path_from_library_root)
        .join("raw");
    // Once we've got a directory-hashing strategy, store dir hash in book_info, and if `book_dump_dir` already exists then hash it, and if it's the same as the stored hash then consider the book already dumped
    for (id, resource) in book.resources.clone() {
        let resource_path = book_dump_dir.join(resource.path);
        match resource_path.starts_with(&book_dump_dir) {
            true => {
                let resource_path_parent = resource_path
                    .parent()
                    .expect("Unreachable: joined path is root.");
                create_dir_all(&resource_path_parent).expect(&format!(
                    "Failed to create directory {}.",
                    resource_path_parent.display()
                ));
                let resource_bytes = book.get_resource(&id).expect("Internal error: EPUB library failed to get resource for id listed in its resources.").0;
                write(&resource_path, resource_bytes)
                    .expect(&format!("Failed to write to {}.", resource_path.display()));
            }
            false => panic!(
                "Error: book contains resource {}, which is attempting a zip slip.",
                resource_path.display()
            ),
        }
    }

    println!(
        "Dumped raw contents of {} to {}.",
        book_path.display(),
        book_dump_dir.display()
    );
}

fn open_book(library: &Library, path: PathBuf) {
    // Once we've got support for multiple formats, do branching here and factor EPUB-handling into its own module.
    let mut book =
        EpubDoc::new(&path).expect(&format!("Couldn't open {} as EPUB.", path.display()));
    let book_info = LibraryBookInfo::from_epub(&library, &mut book);
    dump_raw_book(library, &mut book, &book_info, &path);
}

pub fn open_books(library: &Library, paths: Vec<PathBuf>) {
    // Once we're doing cache-size-limiting this is going to need to get fancier to make sure the cache-update doesn't trim any of the newly-opened books
    for path in paths {
        open_book(library, path);
    }
}
