use std::{
    collections::{HashMap, HashSet},
    fs::{File, create_dir_all, read_to_string, remove_dir_all, write},
    io::BufReader,
    time::SystemTime,
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use cli_table::{Cell, Table};
use epub::doc::EpubDoc;
use serde::{Deserialize, Serialize};

use crate::{
    epub::{EpubInfo, EpubRenditionInfo, SpineNavigationMap, index::EpubIndex, xhtml},
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
    pub library_path: Utf8PathBuf,
    #[serde(skip)]
    index_path: Utf8PathBuf,
    #[serde(default)]
    books: HashMap<String, LibraryBookInfo>,
}

impl Library {
    // Open library

    fn with_paths(self, library_path: Utf8PathBuf, index_path: Utf8PathBuf) -> Self {
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
                        self.index_path
                    ),
                },
                Err(_) => println!(
                    "Warning: failed to serialize library index. Library index may be nonexistent or ill-formed on next program run."
                ),
            },
            Err(_) => println!(
                "Warning: couldn't create library directory {}.",
                self.library_path
            ),
        }
    }

    fn new(library_path: Utf8PathBuf, index_path: Utf8PathBuf) -> Self {
        let new_library = Self {
            library_path,
            index_path,
            books: HashMap::new(),
        };
        new_library.write();
        new_library
    }

    pub fn open(library_dir_path: Utf8PathBuf) -> anyhow::Result<Self> {
        let index_path = library_dir_path.join("library_index.json");
        Ok(match read_to_string(&index_path) {
            Ok(index_string) => match serde_json::from_str::<Self>(&index_string) {
                Ok(library_deserialized) => {
                    library_deserialized.with_paths(library_dir_path, index_path)
                }
                Err(_) => {
                    println!(
                        "Warning: library index at {index_path} is ill-formed. Deleting library and creating new library index."
                    ); // Add y/n prompt for this in case people need the cache for something?
                    remove_dir_all(&library_dir_path).context("Failed to delete library.")?;
                    Self::new(library_dir_path, index_path)
                }
            },
            Err(_) => {
                println!(
                    "Couldn't read library index at {index_path}. Creating new library index."
                );
                Self::new(library_dir_path, index_path)
            }
        })
    }

    // Open books

    pub fn get_internal_path_from_id(&self, id: &str) -> Utf8PathBuf {
        let sanitized_id = sanitize_filename::sanitize(id);

        let mut path_under_consideration = Utf8PathBuf::from(&sanitized_id);
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
            path_under_consideration =
                Utf8PathBuf::from(format!("{sanitized_id}_{numeric_extension}"));
        }

        path_under_consideration
    }

    pub fn register_epub_and_get_id(
        &mut self,
        epub: &mut EpubDoc<BufReader<File>>,
        request_time: SystemTime,
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
            let new_epub_info = EpubInfo::new_from_epub(self, epub, id.clone(), request_time)?;
            self.books
                .insert(id.clone(), LibraryBookInfo::Epub(new_epub_info));
            self.write();
        }
        Ok(id)
    }

    pub fn get_last_opened_styles(&self, id: &str) -> anyhow::Result<&Vec<Style>> {
        let LibraryBookInfo::Epub(epub_info) = self.books.get(id).with_context(|| {
            format!("Couldn't get last opened styles for book id {id}: not found in library index.")
        })?;
        Ok(&epub_info.last_opened_styles)
    }

    fn write_style_index(
        epub_info: &EpubInfo,
        style: &Style,
        library_path: &Utf8Path,
        index_path_from_library_root: &Utf8Path,
        rendition_dir_path: &Utf8Path,
        rendition_contents_dir_path_from_rendition_dir: Utf8PathBuf,
    ) -> anyhow::Result<()> {
        let index =
            EpubIndex::from_spine_and_toc(&epub_info.spine_items, &epub_info.table_of_contents)?;
        let index_xhtml =
            index.to_xhtml(epub_info, rendition_contents_dir_path_from_rendition_dir)?;
        let index_path = library_path.join(index_path_from_library_root);
        write(&index_path, &index_xhtml)
            .with_context(|| format!("Failed to write rendition index to {index_path}."))?;

        let index_stylesheet = crate::epub::index::generate_stylesheet(style)?;
        let index_stylesheet_path = rendition_dir_path.join("index_styles.css");
        write(&index_stylesheet_path, index_stylesheet).with_context(|| {
            format!("Failed to write rendition index stylesheet to {index_stylesheet_path}.")
        })?;

        Ok(())
    }

    fn write_rendition_contents_stylesheets(
        style: &Style,
        rendition_dir_path: &Utf8Path,
    ) -> anyhow::Result<(Option<Utf8PathBuf>, Option<Utf8PathBuf>)> {
        // TODO: have another function for the svg equivalent;
        let (no_override_stylesheet, override_stylesheet) = xhtml::generate_stylesheets(style);

        let no_override_stylesheet_path = match no_override_stylesheet {
            Some(sheet) => {
                let path = rendition_dir_path.join("xhtml_styles_without_override.css");
                write(&path, sheet).with_context(|| {
                    format!("Failed to write rendition no-override stylesheet to {path}.")
                })?;
                Some(path)
            }
            None => None,
        };

        let override_stylesheet_path = match override_stylesheet {
            Some(sheet) => {
                let path = rendition_dir_path.join("xhtml_styles_with_override.css");
                write(&path, sheet).with_context(|| {
                    format!("Failed to write rendition override stylesheet to {path}.")
                })?;
                Some(path)
            }
            None => None,
        };

        Ok((no_override_stylesheet_path, override_stylesheet_path))
    }

    fn link_rendition_contents_nonspine_resources(
        epub_info: &EpubInfo,
        contents_dir_path: &Utf8Path,
        raw_rendition_path: &Utf8Path,
    ) -> anyhow::Result<()> {
        for resource_path in &epub_info.nonspine_resource_paths {
            let resource_link_path = contents_dir_path.join(resource_path);
            let resource_link_path_parent = resource_link_path
                .parent()
                .context("Unreachable: joined path is root.")?;
            create_dir_all(resource_link_path_parent).with_context(|| {
                format!("Failed to create directory {resource_link_path_parent}")
            })?;

            let resource_destination_path = raw_rendition_path.join(resource_path);
            create_link(&resource_link_path, &resource_destination_path)?;
        }
        Ok(())
    }

    fn write_rendition_contents_modified_spine_items(
        epub_info: &EpubInfo,
        style: &Style,
        contents_dir_path: &Utf8Path,
        raw_rendition_path: &Utf8Path,
        no_override_stylesheet_path: &Option<Utf8PathBuf>,
        override_stylesheet_path: &Option<Utf8PathBuf>,
        spine_navigation_maps: &[SpineNavigationMap],
    ) -> anyhow::Result<()> {
        // Todo: add support for SVG spine items
        for spine_item in epub_info.spine_items.iter() {
            let raw_spine_item_path = raw_rendition_path.join(&spine_item.path);
            let modified_spine_item_path = contents_dir_path.join(&spine_item.path);
            let modified_spine_item_xhtml = xhtml::adjust_xhtml_source(
                contents_dir_path,
                &raw_spine_item_path,
                &modified_spine_item_path,
                no_override_stylesheet_path.as_deref(),
                override_stylesheet_path.as_deref(),
                spine_navigation_maps,
                style,
            )?;
            let modified_spine_item_path_parent = modified_spine_item_path
                .parent()
                .context("Unreachable: joined path is root.")?;
            create_dir_all(modified_spine_item_path_parent).with_context(|| {
                format!("Failed to create directory {modified_spine_item_path_parent}")
            })?;
            write(&modified_spine_item_path, modified_spine_item_xhtml)
                .with_context(|| format!("Failed to write file to {modified_spine_item_path}."))?;
        }
        Ok(())
    }

    fn write_navigation(
        epub_info: &EpubInfo,
        style: &Style,
        rendition_dir_path: &Utf8Path,
        spine_navigation_maps: &[SpineNavigationMap],
    ) -> anyhow::Result<()> {
        for (spine_index, spine_navigation_map) in spine_navigation_maps.iter().enumerate() {
            let section_path =
                Utf8Path::new("contents").join(&spine_navigation_map.spine_item.path);
            let navigation_contents_wrapped =
                xhtml::wrap_xhtml_source_for_navigation(rendition_dir_path, &section_path)?;
            let navigation_file = crate::epub::navigation::create_navigation_file(
                epub_info,
                spine_navigation_maps,
                spine_index,
                style,
                &navigation_contents_wrapped,
            )?;
            let navigation_file_path =
                rendition_dir_path.join(&spine_navigation_map.navigation_filename);
            write(&navigation_file_path, navigation_file).with_context(|| {
                format!(
                    "Failed to write rendition navigation stylesheet to {navigation_file_path}."
                )
            })?;
        }

        let navigation_stylesheet = crate::epub::navigation::generate_stylesheet(style)?;
        let navigation_stylesheet_path = rendition_dir_path.join("navigation_styles.css");
        write(&navigation_stylesheet_path, navigation_stylesheet).with_context(|| {
            format!(
                "Failed to write rendition navigation stylesheet to {navigation_stylesheet_path}."
            )
        })?;

        let navigation_script = include_str!("../assets/navigation_script.js");
        let navigation_script_path = rendition_dir_path.join("navigation_script.js");
        write(&navigation_script_path, navigation_script).with_context(|| {
            format!("Failed to write rendition navigation script to {navigation_script_path}.")
        })?;

        Ok(())
    }

    fn generate_rendition_contents_dir(
        epub_info: &EpubInfo,
        style: &Style,
        rendition_dir_path: &Utf8Path,
        raw_rendition_path: &Utf8Path,
    ) -> anyhow::Result<()> {
        let contents_dir_path = rendition_dir_path.join("contents");

        let (no_override_stylesheet_path, override_stylesheet_path) =
            Self::write_rendition_contents_stylesheets(style, rendition_dir_path)?;
        let spine_navigation_maps = epub_info.get_spine_navigation_maps();

        Self::link_rendition_contents_nonspine_resources(
            epub_info,
            &contents_dir_path,
            raw_rendition_path,
        )?;
        Self::write_rendition_contents_modified_spine_items(
            epub_info,
            style,
            &contents_dir_path,
            raw_rendition_path,
            &no_override_stylesheet_path,
            &override_stylesheet_path,
            &spine_navigation_maps,
        )?;

        if style.inject_navigation {
            Self::write_navigation(epub_info, style, rendition_dir_path, &spine_navigation_maps)?;
        }

        Ok(())
    }

    pub fn register_book_styles(&mut self, id: &str, styles: &[Style]) -> anyhow::Result<()> {
        let LibraryBookInfo::Epub(epub_info) = self.books.get_mut(id).with_context(|| {
            format!("Couldn't register styles for book id {id}: not found in library index.")
        })?;
        let mut write_needed = false;
        for style in styles {
            if epub_info.find_rendition(style).is_none() {
                write_needed = true;

                let rendition_dir_path_from_library_root =
                    epub_info.get_new_rendition_dir_path_from_style(style);
                let rendition_dir_path = self
                    .library_path
                    .join(&rendition_dir_path_from_library_root);
                create_dir_all(&rendition_dir_path)
                    .context("Couldn't create rendition directory for new style.")?;

                let default_file_path_from_library_root =
                    match (style.include_index, style.uses_raw_contents_dir()) {
                        (true, true) => {
                            let index_path_from_library_root =
                                rendition_dir_path_from_library_root.join("index.xhtml");
                            Self::write_style_index(
                                epub_info,
                                style,
                                &self.library_path,
                                &index_path_from_library_root,
                                &rendition_dir_path,
                                ["..", "raw"].iter().collect(),
                            )?;
                            index_path_from_library_root
                        }
                        (true, false) => {
                            let index_path_from_library_root =
                                rendition_dir_path_from_library_root.join("index.xhtml");
                            Self::write_style_index(
                                epub_info,
                                style,
                                &self.library_path,
                                &index_path_from_library_root,
                                &rendition_dir_path,
                                "contents".into(),
                            )?;
                            Self::generate_rendition_contents_dir(
                                epub_info,
                                style,
                                &rendition_dir_path,
                                &self
                                    .library_path
                                    .join(&epub_info.raw_rendition.dir_path_from_library_root),
                            )?;
                            index_path_from_library_root
                        }
                        (false, true) => epub_info
                            .raw_rendition
                            .default_file_path_from_library_root
                            .clone(),
                        (false, false) => {
                            Self::generate_rendition_contents_dir(
                                epub_info,
                                style,
                                &rendition_dir_path,
                                &self
                                    .library_path
                                    .join(&epub_info.raw_rendition.dir_path_from_library_root),
                            )?;
                            rendition_dir_path_from_library_root
                                .join("contents")
                                .join(&epub_info.first_linear_spine_item_path)
                        }
                    };

                epub_info.nonraw_renditions.push(EpubRenditionInfo {
                    style: style.clone(),
                    dir_path_from_library_root: rendition_dir_path_from_library_root,
                    default_file_path_from_library_root,
                    bytes: get_dir_size(rendition_dir_path.as_ref())?,
                });
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
        request_time: SystemTime,
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
        target_rendition.open_in_browser(self.library_path.as_ref(), browser)?;
        match epub_info.last_opened_time == request_time {
            true => epub_info.last_opened_styles.push(style.clone()),
            false => {
                epub_info.last_opened_time = request_time;
                epub_info.last_opened_styles = vec![style.clone()];
            }
        }
        self.write();
        Ok(())
    }

    // Manage library

    pub fn list(&self) -> anyhow::Result<()> {
        // Maybe give this more styling later; but it's good enough for now.
        let mut books_vec = self.books.iter().collect::<Vec<_>>();
        books_vec.sort_by_key(|(_id, LibraryBookInfo::Epub(epub_info))| &epub_info.title);
        let table = books_vec
            .into_iter()
            .map(|(id, LibraryBookInfo::Epub(epub_info))| [id.cell(), (&epub_info.title).cell()])
            .collect::<Vec<_>>()
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
                format!("Failed to remove {} from {book_dir}.", epub_info.title,)
            })?;
        } // If it exists but isn't a dir, maybe have handling for that to avoid later messes?
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

            let mut unexcluded_ids = self
                .books
                .iter()
                .filter(|(id, _book_info)| !ids_to_exclude.contains(*id))
                .collect::<Vec<_>>();
            unexcluded_ids.sort_unstable_by(
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
            );
            let mut ids_to_potentially_remove = unexcluded_ids
                .into_iter()
                .map(|(id, _book_info)| id)
                .rev()
                .cloned()
                .collect::<Vec<_>>();

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
