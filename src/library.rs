use std::{
    fs::{File, create_dir_all, read_to_string, remove_dir_all, write}, io::BufReader, path::PathBuf
};

use epub::doc::EpubDoc;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBookInfo {
    id: String,
    title: String,
    creators: Vec<String>,
    path_from_library_root: PathBuf,
    // bytes: u64
}

impl LibraryBookInfo {
	pub fn from_epub(library: &Library, epub: &EpubDoc<BufReader<File>>) -> Self {
		let id = match epub.get_release_identifier() {
			Some(id) => id,
			None => epub.unique_identifier.as_ref().expect("Ill-formed EPUB: no unique identifier.").clone(),
		};
		let title = epub.get_title().expect("Ill-formed EPUB: no title.");
		let creators = epub.metadata.iter().filter_map(|metadata_item| {
			match &metadata_item.property == "creator" {
				true => Some(metadata_item.value.clone()),
				false => None,
			}
		}).collect();
		let path_from_library_root = library.get_internal_path_from_id(&id);
		Self {
			id,
			title,
			creators,
			path_from_library_root,
		}
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    #[serde(skip)]
    library_path: PathBuf,
    #[serde(skip)]
    index_path: PathBuf,
    #[serde(default)]
    books: Vec<LibraryBookInfo>,
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
					Err(_) => println!("Warning: failed to write library index. Library may be cleared on next program run."),
				}
				Err(_) => println!("Warning: failed to serialize library index. Library may be cleared on next program run."),
			}
			Err(_) => println!("Warning: couldn't create library directory."),
		}
    }

    fn new(library_path: PathBuf, index_path: PathBuf) -> Self {
        let new_cache = Self {
            library_path,
            index_path,
            books: Vec::new(),
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
                    println!("Warning: library index is ill-formed. Clearing library and creating new library index."); // Add y/n prompt for this in case people need the cache for something?
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
		while self.books.iter().any(|book| book.path_from_library_root == path_under_consideration && book.id != sanitized_id ) {
			numeric_extension += 1;
			path_under_consideration = PathBuf::from(format!("{sanitized_id}_{numeric_extension}"));
		}

		path_under_consideration
	}
}
