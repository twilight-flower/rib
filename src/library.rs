use std::{
    fs::{create_dir_all, read_to_string, remove_dir_all, write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LibraryBookMetadata {
    id: String,
    title: String,
    creators: Vec<String>,
    path_from_library_root: PathBuf,
    // bytes: u64
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    #[serde(skip)]
    library_path: PathBuf,
    #[serde(skip)]
    index_path: PathBuf,
    #[serde(default)]
    books: Vec<LibraryBookMetadata>,
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
}
