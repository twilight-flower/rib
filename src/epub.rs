use std::{
    fs::{File, create_dir_all, write},
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
    library::Library,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubSpineItem {
    pub path: PathBuf,
    pub linear: bool,
    // properties can go here later, but ignore them for now
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubRenditionInfo {
    pub dir_path_from_library_root: PathBuf,
    pub default_file_path_from_library_root: PathBuf,
    pub bytes: u64,
}

impl EpubRenditionInfo {
    pub fn open_in_browser(&self, library_path: &PathBuf, browser: &Option<String>) {
        let path_to_canonicalize = library_path.join(&self.default_file_path_from_library_root);
        let path_to_open = path_to_canonicalize
            .canonicalize() // To help cross-platform compatibility, hopefully
            .expect(&format!(
                "Unable to canonicalize book rendition path {} to open.",
                path_to_canonicalize.display()
            ));
        browser::open(&path_to_open, browser);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubInfo {
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

    // Contents
    pub raw_spine_items: Vec<EpubSpineItem>,
    pub raw_nonspine_resource_paths: Vec<PathBuf>,

    // Renditions
    pub raw_rendition: EpubRenditionInfo,
    // pub renditions: Vec<LibraryBookRenditionInfo>,
}

impl EpubInfo {
    pub fn new_from_epub(
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

        println!(
            "Dumped raw contents of {} to {}.",
            epub_path.display(),
            raw_dir.display()
        );

        let raw_spine_items = epub.spine.iter().map(|spine_item| EpubSpineItem {
            path: standardize_pathbuf_separators(&raw_dir_path_from_library_root.join(&epub.resources.get(&spine_item.idref).expect("Internal error: EPUB library failed to get resource for id listed in its spine.").path)),
            linear: spine_item.linear,
        }).collect_vec();
        let raw_nonspine_resource_paths = epub
            .resources
            .values()
            .filter_map(|resource| {
                let resource_path = raw_dir_path_from_library_root.join(&resource.path);
                match raw_spine_items
                    .iter()
                    .any(|spine_item| spine_item.path == resource_path)
                {
                    true => None,
                    false => Some(standardize_pathbuf_separators(&resource_path)),
                }
            })
            .collect_vec();

        let first_linear_raw_spine_item_path = standardize_pathbuf_separators(
            &raw_spine_items
                .iter()
                .find(|item| item.linear)
                .expect("Ill-formed EPUB: no linear spine items.")
                .path,
        );

        Self {
            id: epub_id,
            title: epub.get_title().expect("Ill-formed EPUB: no title."),
            path_from_library_root,
            added_time: request_time,
            last_opened_time: request_time,
            raw_spine_items,
            raw_nonspine_resource_paths,
            raw_rendition: EpubRenditionInfo {
                default_file_path_from_library_root: first_linear_raw_spine_item_path,
                dir_path_from_library_root: raw_dir_path_from_library_root,
                bytes: get_dir_size(&raw_dir),
            },
        }
    }

    pub fn size_in_bytes(&self) -> u64 {
        // Will need to be complexified once we add support for non-raw renditions
        self.raw_rendition.bytes
    }
}
