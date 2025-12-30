use std::{
    collections::HashMap,
    fs::{File, create_dir_all, read_to_string, remove_dir_all, write},
    io::BufReader,
    path::PathBuf,
};

use chrono::{DateTime, Utc};
use epub::doc::EpubDoc;
use serde::{Deserialize, Serialize};

use crate::browser;
use crate::helpers::{deserialize_datetime, serialize_datetime};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBookRenditionInfo {
    pub dir_path_from_library_root: PathBuf,
    pub file_path_from_library_root: PathBuf,
}

impl LibraryBookRenditionInfo {
    pub fn open_in_browser(&self, library_path: &PathBuf) {
        let path_to_open = library_path
            .join(&self.file_path_from_library_root)
            .canonicalize()
            .expect("Unable to canonicalize book rendition path to open.");
        browser::open(&path_to_open);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBookInfo {
    // Identifiers
    pub id: String,
    pub title: String,

    // Library-management-relevant metadata
    pub path_from_library_root: PathBuf,
    #[serde(
        deserialize_with = "deserialize_datetime",
        serialize_with = "serialize_datetime"
    )]
    pub added_time: DateTime<Utc>,
    #[serde(
        deserialize_with = "deserialize_datetime",
        serialize_with = "serialize_datetime"
    )]
    pub last_opened_time: DateTime<Utc>,
    // bytes: u64

    // Renditions
    pub raw_rendition: LibraryBookRenditionInfo,
    // pub renditions: Vec<LibraryBookRenditionInfo>,
}

impl LibraryBookInfo {
    fn new_from_epub(
        library: &mut Library,
        epub: &mut EpubDoc<BufReader<File>>,
        epub_id: String,
        epub_path: &PathBuf,
    ) -> Self {
        let path_from_library_root = library.get_internal_path_from_id(&epub_id);
        let raw_dir_path_from_library_root = path_from_library_root.join("raw");
        let raw_dir = library.library_path.join(&raw_dir_path_from_library_root);
        for (id, resource) in epub.resources.clone() {
            let resource_path = raw_dir.join(resource.path);
            match resource_path.starts_with(&raw_dir) {
                true => {
                    let resource_path_parent = resource_path
                        .parent()
                        .expect("Unreachable: joined path is root.");
                    create_dir_all(&resource_path_parent).expect(&format!(
                        "Failed to create directory {}.",
                        resource_path_parent.display()
                    ));
                    let resource_bytes = epub.get_resource(&id).expect("Internal error: EPUB library failed to get resource for id listed in its resources.").0;
                    write(&resource_path, resource_bytes)
                        .expect(&format!("Failed to write to {}.", resource_path.display()));
                }
                false => panic!(
                    "Book contains resource {}, which is attempting a zip slip.",
                    resource_path.display()
                ),
            }
        }

        let first_linear_spine_item_idref = &epub
            .spine
            .iter()
            .find(|item| item.linear)
            .expect("Ill-formed EPUB: no linear spine items.")
            .idref;
        let first_linear_spine_item_path = &epub
            .resources
            .get(first_linear_spine_item_idref)
            .expect(
                "Internal error: EPUB library failed to get resource for id listed in its spine.",
            )
            .path;

        println!(
            "Dumped raw contents of {} to {}.",
            epub_path.display(),
            raw_dir.display()
        );

        let now = Utc::now();
        Self {
            id: epub_id,
            title: epub.get_title().expect("Ill-formed EPUB: no title."),
            path_from_library_root,
            added_time: now,
            last_opened_time: now,
            raw_rendition: LibraryBookRenditionInfo {
                file_path_from_library_root: raw_dir_path_from_library_root
                    .join(first_linear_spine_item_path),
                dir_path_from_library_root: raw_dir_path_from_library_root,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    #[serde(skip)]
    pub library_path: PathBuf,
    #[serde(skip)]
    index_path: PathBuf,
    #[serde(default)]
    books: HashMap<String, LibraryBookInfo>,
    // max_books: Option<u64>,
    // max_bytes: Option<u64>,
}

impl Library {
    fn with_paths(self, library_path: PathBuf, index_path: PathBuf) -> Self {
        Self {
            library_path,
            index_path,
            ..self
        }
    }

    fn write(&self) {
        match create_dir_all(&self.library_path) {
            Ok(_) => match serde_json::to_string_pretty(self) {
                Ok(self_serialized) => match write(&self.index_path, self_serialized) {
                    Ok(_) => (),
                    Err(_) => println!(
                        "Warning: failed to write library index. Library may be cleared on next program run."
                    ),
                },
                Err(_) => println!(
                    "Warning: failed to serialize library index. Library may be cleared on next program run."
                ),
            },
            Err(_) => println!("Warning: couldn't create library directory."),
        }
    }

    fn new(library_path: PathBuf, index_path: PathBuf) -> Self {
        let new_cache = Self {
            library_path,
            index_path,
            books: HashMap::new(),
        };
        new_cache.write();
        new_cache
    }

    pub fn open(library_path: PathBuf) -> Self {
        let index_path = library_path.join("library_index.json");
        match read_to_string(&index_path) {
            Ok(index_string) => match serde_json::from_str::<Self>(&index_string) {
                Ok(library_deserialized) => {
                    library_deserialized.with_paths(library_path, index_path)
                }
                Err(_) => {
                    println!(
                        "Warning: library index is ill-formed. Clearing library and creating new library index."
                    ); // Add y/n prompt for this in case people need the cache for something?
                    remove_dir_all(&library_path).expect("Failed to clear library.");
                    Self::new(library_path, index_path)
                }
            },
            Err(_) => {
                println!("Couldn't read library index. Creating new library index.");
                Self::new(library_path, index_path)
            }
        }
    }

    fn get_internal_path_from_id(&self, id: &str) -> PathBuf {
        let sanitized_id = sanitize_filename::sanitize(id);

        let mut path_under_consideration = PathBuf::from(&sanitized_id);
        let mut numeric_extension = 1;
        while self.books.iter().any(|(book_id, book_info)| {
            book_info.path_from_library_root == path_under_consideration && book_id != &sanitized_id
        }) {
            numeric_extension += 1;
            path_under_consideration = PathBuf::from(format!("{sanitized_id}_{numeric_extension}"));
        }

        path_under_consideration
    }

    pub fn register_epub_and_get_id(
        &mut self,
        epub: &mut EpubDoc<BufReader<File>>,
        epub_path: &PathBuf,
    ) -> String {
        let id = match epub.get_release_identifier() {
            Some(id) => id,
            None => epub
                .unique_identifier
                .as_ref()
                .expect("Ill-formed EPUB: no unique identifier.")
                .clone(),
        };
        if !self.books.contains_key(&id) {
            let new_book_info = LibraryBookInfo::new_from_epub(self, epub, id.clone(), epub_path);
            self.books.insert(id.clone(), new_book_info);
            self.write();
        }
        id
    }

    pub fn open_raw(&mut self, id: &str) {
        let book_info = self
            .books
            .get_mut(id)
            .expect(&format!("Couldn't open book id {id}: not found."));
        book_info.raw_rendition.open_in_browser(&self.library_path);
        book_info.last_opened_time = Utc::now();
        self.write();
    }
}
