use std::{
    collections::{HashMap, HashSet},
    fs::{File, create_dir_all, read_to_string, remove_dir_all, write},
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use cli_table::{Cell, Table};
use epub::doc::EpubDoc;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    epub::{EpubInfo, EpubRenditionInfo, index::EpubIndex, xhtml::adjust_spine_xhtml},
    helpers::{create_link, get_dir_size},
    style::Style,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
enum LibraryBookInfo {
    // Might be nice to implement a trait for all book-info types once there's more than one, for API-consistency
    Epub(EpubInfo),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    #[serde(skip)]
    pub library_path: PathBuf,
    #[serde(skip)]
    index_path: PathBuf,
    #[serde(default)]
    books: HashMap<String, LibraryBookInfo>,
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

    pub fn open(library_dir_path: PathBuf) -> anyhow::Result<Self> {
        let index_path = library_dir_path.join("library_index.json");
        Ok(match read_to_string(&index_path) {
            Ok(index_string) => match serde_json::from_str::<Self>(&index_string) {
                Ok(library_deserialized) => {
                    library_deserialized.with_paths(library_dir_path, index_path)
                }
                Err(_) => {
                    println!(
                        "Warning: library index at {} is ill-formed. Deleting library and creating new library index.",
                        index_path.display()
                    ); // Add y/n prompt for this in case people need the cache for something?
                    remove_dir_all(&library_dir_path).context("Failed to delete library.")?;
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
        })
    }

    // Open books

    pub fn get_internal_path_from_id(&self, id: &str) -> PathBuf {
        let sanitized_id = sanitize_filename::sanitize(id);

        let mut path_under_consideration = PathBuf::from(&sanitized_id);
        let mut numeric_extension = 1;
        while self
            .books
            .iter()
            .any(|(book_id, LibraryBookInfo::Epub(epub_info))| {
                epub_info.path_from_library_root == path_under_consideration
                    && book_id != &sanitized_id
            })
        {
            numeric_extension += 1;
            path_under_consideration = PathBuf::from(format!("{sanitized_id}_{numeric_extension}"));
        }

        path_under_consideration
    }

    pub fn register_epub_and_get_id(
        &mut self,
        epub: &mut EpubDoc<BufReader<File>>,
        epub_path: &Path,
        request_time: DateTime<Utc>,
    ) -> anyhow::Result<String> {
        let id = match epub.get_release_identifier() {
            Some(id) => id,
            None => epub
                .unique_identifier
                .as_ref()
                .context("Ill-formed EPUB: no unique identifier.")?
                .clone(),
        };
        if !self.books.contains_key(&id) {
            let new_epub_info =
                EpubInfo::new_from_epub(self, epub, id.clone(), epub_path, request_time)?;
            self.books
                .insert(id.clone(), LibraryBookInfo::Epub(new_epub_info));
            self.write();
        }
        Ok(id)
    }

    pub fn register_book_styles(&mut self, id: &str, styles: &[Style]) -> anyhow::Result<()> {
        let LibraryBookInfo::Epub(epub_info) = self.books.get_mut(id).with_context(|| {
            format!("Couldn't register styles for book id {id}: not found in library index.")
        })?;
        let mut write_needed = false;
        for style in styles {
            let style_already_present = match style == &Style::raw() {
                true => true,
                false => epub_info.find_rendition(style).is_some(),
            };
            if !style_already_present {
                let dir_path_from_library_root =
                    epub_info.get_new_rendition_dir_path_from_style(style);
                let dir_path = self.library_path.join(&dir_path_from_library_root);
                create_dir_all(&dir_path)
                    .context("Couldn't create rendition directory for new style.")?;

                if style.inject_navigation {
                    // Will also need to branch on stylesheet stuff later
                    let raw_rendition_path = self
                        .library_path
                        .join(&epub_info.raw_rendition.dir_path_from_library_root);
                    let contents_dir = dir_path.join("contents");

                    for resource_path in &epub_info.nonspine_resource_paths {
                        let resource_link_path = contents_dir.join(resource_path);
                        let resource_link_path_parent = resource_link_path
                            .parent()
                            .context("Unreachable: joined path is root.")?;
                        create_dir_all(&resource_link_path_parent).with_context(|| {
                            format!(
                                "Failed to create directory {}",
                                resource_link_path_parent.display()
                            )
                        })?;

                        let resource_destination_path = raw_rendition_path.join(resource_path);
                        create_link(&resource_link_path, &resource_destination_path)?;
                    }
                    for (index, spine_item) in epub_info.spine_items.iter().enumerate() {
                        // Todo: add support for SVG spine items
                        let raw_spine_item_path = raw_rendition_path.join(&spine_item.path);
                        let modified_spine_item_path = contents_dir.join(&spine_item.path);
                        let modified_spine_item_xhtml = adjust_spine_xhtml(
                            &epub_info,
                            &contents_dir,
                            &raw_spine_item_path,
                            &modified_spine_item_path,
                            index,
                            style,
                        )?;
                        let modified_spine_item_path_parent = modified_spine_item_path
                            .parent()
                            .context("Unreachable: joined path is root.")?;
                        create_dir_all(&modified_spine_item_path_parent).with_context(|| {
                            format!(
                                "Failed to create directory {}",
                                modified_spine_item_path_parent.display()
                            )
                        })?;
                        write(&modified_spine_item_path, modified_spine_item_xhtml).with_context(
                            || {
                                format!(
                                    "Failed to write file to {}.",
                                    modified_spine_item_path.display()
                                )
                            },
                        )?;
                    }

                    let navigation_stylesheet = crate::epub::navigation::generate_stylesheet(style);
                    let navigation_stylesheet_path = dir_path.join("navigation_styles.css");
                    write(&navigation_stylesheet_path, navigation_stylesheet).with_context(
                        || {
                            format!(
                                "Failed to write rendition navigation stylesheet to {}.",
                                navigation_stylesheet_path.display()
                            )
                        },
                    )?;
                }

                let default_file_path_from_library_root = match style.include_index {
                    true => {
                        let index = EpubIndex::from_spine_and_toc(
                            &epub_info.spine_items,
                            &epub_info.table_of_contents,
                        )?;
                        let index_xhtml = index.to_xhtml(epub_info, style)?;
                        let index_path_from_library_root =
                            dir_path_from_library_root.join("index.xhtml");
                        let index_path = self.library_path.join(&index_path_from_library_root);
                        write(&index_path, &index_xhtml).with_context(|| {
                            format!(
                                "Failed to write rendition index to {}.",
                                index_path.display()
                            )
                        })?;

                        // Index stylesheet will need to be generated more dynamically once we've got user-supplied styling enabled
                        let index_stylesheet = crate::epub::index::generate_stylesheet(style);
                        let index_stylesheet_path = dir_path.join("index_styles.css");
                        write(&index_stylesheet_path, index_stylesheet).with_context(|| {
                            format!(
                                "Failed to write rendition index stylesheet to {}.",
                                index_stylesheet_path.display()
                            )
                        })?;

                        index_path_from_library_root
                    }
                    false => match style.uses_raw_contents_dir() {
                        // This is slightly inefficient, calculating the contents dir redundantly. But would be hard to convince the compiler to go along with defining it elsewhere more efficiently, and this isn't *that* bad, so here it is.
                        true => epub_info
                            .raw_rendition
                            .default_file_path_from_library_root
                            .clone(),
                        false => dir_path_from_library_root
                            .join("contents")
                            .join(&epub_info.first_linear_spine_item_path),
                    },
                };

                epub_info.nonraw_renditions.push(EpubRenditionInfo {
                    style: style.clone(),
                    dir_path_from_library_root,
                    default_file_path_from_library_root,
                    bytes: get_dir_size(&dir_path)?,
                });

                write_needed = true;
            }
        }
        if write_needed {
            self.write();
        }
        Ok(())
    }

    pub fn open_book(
        &mut self,
        id: &str,
        request_time: DateTime<Utc>,
        browser: &Option<String>,
        style: &Style,
    ) -> anyhow::Result<()> {
        let LibraryBookInfo::Epub(epub_info) = self
            .books
            .get_mut(id)
            .with_context(|| format!("Couldn't open book id {id}: not found in library index."))?;
        let target_rendition = epub_info.find_rendition(style).with_context(|| {
            format!("Internal error: tried to open book id {id} with an unregistered style.")
        })?;
        target_rendition.open_in_browser(&self.library_path, browser)?;
        epub_info.last_opened_time = request_time;
        self.write();
        Ok(())
    }

    // Manage library

    pub fn list(&self) -> anyhow::Result<()> {
        // Maybe give this more styling later; but it's good enough for now.
        let table = self
            .books
            .iter()
            .sorted_by_key(|(_id, LibraryBookInfo::Epub(epub_info))| &epub_info.title)
            .map(|(id, LibraryBookInfo::Epub(epub_info))| [id.cell(), (&epub_info.title).cell()])
            .collect_vec()
            .table()
            .title(["ID".cell(), "Title".cell()]);
        println!(
            "{}",
            table
                .display()
                .context("Couldn't display library list table.")?
        );
        Ok(())
    }

    fn size_in_bytes(&self) -> u64 {
        self.books
            .values()
            .fold(0, |size_sum, LibraryBookInfo::Epub(epub_info)| {
                size_sum + epub_info.size_in_bytes()
            })
    }

    fn is_oversized(&self, max_books: Option<usize>, max_bytes: Option<u64>) -> bool {
        let too_many_books =
            max_books.is_some_and(|max_books_unwrapped| self.books.len() > max_books_unwrapped);
        let too_many_bytes =
            max_bytes.is_some_and(|max_bytes_unwrapped| self.size_in_bytes() > max_bytes_unwrapped);
        too_many_books || too_many_bytes
    }

    fn remove_book(&mut self, id: &str) -> anyhow::Result<()> {
        let LibraryBookInfo::Epub(epub_info) = self.books.remove(id).with_context(|| {
            format!("Couldn't remove book id {id}: not found in library index.")
        })?;
        let book_dir = self.library_path.join(&epub_info.path_from_library_root);
        if book_dir.is_dir() {
            remove_dir_all(&book_dir).with_context(|| {
                format!(
                    "Failed to remove {} from {}.",
                    epub_info.title,
                    book_dir.display()
                )
            })?;
        } // If it exists but isn't a dir, maybe have handling for that to avoid later messes?
        println!("Removed {} from {}.", epub_info.title, book_dir.display());
        Ok(())
    }

    pub fn truncate(
        &mut self,
        max_books: Option<usize>,
        max_bytes: Option<u64>,
        ids_to_exclude: &HashSet<String>,
    ) -> anyhow::Result<()> {
        let mut oversized = self.is_oversized(max_books, max_bytes);
        if oversized {
            let mut write_needed = false;

            let mut ids_to_potentially_remove = self
                .books
                .iter()
                .filter(|(id, _book_info)| !ids_to_exclude.contains(*id))
                .sorted_unstable_by(
                    |(_id_1, LibraryBookInfo::Epub(epub_info_1)),
                     (_id_2, LibraryBookInfo::Epub(epub_info_2))| {
                        match epub_info_1
                            .last_opened_time
                            .cmp(&epub_info_2.last_opened_time)
                        {
                            std::cmp::Ordering::Equal => epub_info_2
                                .size_in_bytes()
                                .cmp(&epub_info_1.size_in_bytes()),
                            nonequal_datetime_ordering => nonequal_datetime_ordering,
                        }
                    },
                )
                .map(|(id, _book_info)| id)
                .rev()
                .cloned()
                .collect_vec();

            while oversized && let Some(id) = ids_to_potentially_remove.pop() {
                self.remove_book(&id)?;
                write_needed = true;
                oversized = self.is_oversized(max_books, max_bytes);
            }

            if write_needed {
                self.write();
            }
        }
        Ok(())
    }

    pub fn clear(
        &mut self,
        max_books: Option<usize>,
        max_bytes: Option<u64>,
        target_ids: &[String],
    ) -> anyhow::Result<()> {
        if !target_ids.is_empty() {
            for id in target_ids {
                self.remove_book(id)?;
            }
            self.write();
        }
        self.truncate(max_books, max_bytes, &HashSet::new())?;
        Ok(())
    }
}
