use std::{
    collections::{HashMap, HashSet},
    fs::{File, create_dir_all, read_to_string, remove_dir_all, write},
    io::BufReader,
    path::PathBuf,
};

use chrono::{DateTime, Utc};
use epub::doc::EpubDoc;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    browser,
    helpers::{
        deserialize_datetime, get_dir_size, serialize_datetime, standardize_pathbuf_separators,
    },
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBookRenditionInfo {
    pub dir_path_from_library_root: PathBuf,
    pub file_path_from_library_root: PathBuf,
    pub bytes: u64,
}

impl LibraryBookRenditionInfo {
    pub fn open_in_browser(&self, library_path: &PathBuf, browser: &Option<String>) {
        let path_to_open = library_path
            .join(&self.file_path_from_library_root)
            .canonicalize() // To help cross-platform compatibility, hopefully
            .expect("Unable to canonicalize book rendition path to open.");
        browser::open(&path_to_open, browser);
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
        request_time: DateTime<Utc>,
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
        let first_linear_spine_item_path = standardize_pathbuf_separators(&epub
            .resources
            .get(first_linear_spine_item_idref)
            .expect(
                "Internal error: EPUB library failed to get resource for id listed in its spine.",
            )
            .path);

        println!(
            "Dumped raw contents of {} to {}.",
            epub_path.display(),
            raw_dir.display()
        );

        let raw_rendition_dir_size = get_dir_size(&raw_dir);
        Self {
            id: epub_id,
            title: epub.get_title().expect("Ill-formed EPUB: no title."),
            path_from_library_root,
            added_time: request_time,
            last_opened_time: request_time,
            raw_rendition: LibraryBookRenditionInfo {
                file_path_from_library_root: raw_dir_path_from_library_root
                    .join(first_linear_spine_item_path),
                dir_path_from_library_root: raw_dir_path_from_library_root,
                bytes: raw_rendition_dir_size,
            },
        }
    }

    fn size_in_bytes(&self) -> u64 {
        // Will need to be complexified once we add support for non-raw renditions
        self.raw_rendition.bytes
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
    // Open library

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
                        "Warning: failed to write library index to {}. Library index may be nonexistent or ill-formed on next program run.",
                        self.index_path.display()
                    ),
                },
                Err(_) => println!(
                    "Warning: failed to serialize library index. Library index may be nonexistent or ill-formed on next program run."
                ),
            },
            Err(_) => println!(
                "Warning: couldn't create library directory {}.",
                self.library_path.display()
            ),
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

    pub fn open(library_dir_path: PathBuf) -> Self {
        let index_path = library_dir_path.join("library_index.json");
        match read_to_string(&index_path) {
            Ok(index_string) => match serde_json::from_str::<Self>(&index_string) {
                Ok(library_deserialized) => {
                    library_deserialized.with_paths(library_dir_path, index_path)
                }
                Err(_) => {
                    println!(
                        "Warning: library index at {} is ill-formed. Clearing library and creating new library index.",
                        index_path.display()
                    ); // Add y/n prompt for this in case people need the cache for something?
                    remove_dir_all(&library_dir_path).expect("Failed to clear library.");
                    Self::new(library_dir_path, index_path)
                }
            },
            Err(_) => {
                println!(
                    "Couldn't read library index at {}. Creating new library index.",
                    index_path.display()
                );
                Self::new(library_dir_path, index_path)
            }
        }
    }

    // Open books

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
        request_time: DateTime<Utc>,
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
            let new_book_info =
                LibraryBookInfo::new_from_epub(self, epub, id.clone(), epub_path, request_time);
            self.books.insert(id.clone(), new_book_info);
            self.write();
        }
        id
    }

    pub fn open_book_raw(
        &mut self,
        id: &str,
        request_time: DateTime<Utc>,
        browser: &Option<String>,
    ) {
        let book_info = self.books.get_mut(id).expect(&format!(
            "Couldn't open book id {id}: not found in library index."
        ));
        book_info
            .raw_rendition
            .open_in_browser(&self.library_path, browser);
        book_info.last_opened_time = request_time;
        self.write();
    }

    // Manage library

    fn size_in_bytes(&self) -> u64 {
        self.books.values().fold(0, |size_sum, book_info| {
            size_sum + book_info.size_in_bytes()
        })
    }

    fn is_oversized(&self, max_books: Option<usize>, max_bytes: Option<u64>) -> bool {
        let too_many_books =
            max_books.is_some_and(|max_books_unwrapped| self.books.len() > max_books_unwrapped);
        let too_many_bytes =
            max_bytes.is_some_and(|max_bytes_unwrapped| self.size_in_bytes() > max_bytes_unwrapped);
        too_many_books || too_many_bytes
    }

    fn remove_book(&mut self, id: &str) {
        let book_info = self.books.remove(id).expect(&format!(
            "Couldn't remove book id {id}: not found in library index."
        ));
        let book_dir = self.library_path.join(&book_info.path_from_library_root);
        if book_dir.is_dir() {
            remove_dir_all(&book_dir).expect(&format!(
                "Failed to remove {} from {}.",
                book_info.title,
                book_dir.display()
            ));
        } // If it exists but isn't a dir, maybe have handling for that to avoid later messes?
        println!("Removed {} from {}.", book_info.title, book_dir.display());
    }

    pub fn truncate(
        &mut self,
        max_books: Option<usize>,
        max_bytes: Option<u64>,
        ids_to_exclude: &HashSet<String>,
    ) {
        let mut oversized = self.is_oversized(max_books, max_bytes);
        if oversized {
            let mut write_needed = false;

            let mut ids_to_potentially_remove = self
                .books
                .iter()
                .filter(|(id, _book_info)| !ids_to_exclude.contains(*id))
                .sorted_unstable_by(
                    |(_id_1, book_info_1), (_id_2, book_info_2)| match book_info_1
                        .last_opened_time
                        .cmp(&book_info_2.last_opened_time)
                    {
                        std::cmp::Ordering::Equal => book_info_2
                            .size_in_bytes()
                            .cmp(&book_info_1.size_in_bytes()),
                        nonequal_datetime_ordering => nonequal_datetime_ordering,
                    },
                )
                .map(|(id, _book_info)| id)
                .rev()
                .cloned()
                .collect_vec();

            while oversized && let Some(id) = ids_to_potentially_remove.pop() {
                self.remove_book(&id);
                write_needed = true;
                oversized = self.is_oversized(max_books, max_bytes);
            }

            if write_needed {
                self.write();
            }
        }
    }
}
