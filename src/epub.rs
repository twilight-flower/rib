pub mod index;
pub mod navigation;
pub mod xhtml;

use std::{
    fs::{File, create_dir_all, write},
    io::BufReader,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::SystemTime,
};

use anyhow::{Context, bail};
use epub::doc::EpubDoc;
use path_clean::PathClean;
use serde::{Deserialize, Serialize};

use crate::{
    browser,
    helpers::{get_dir_size, standardize_path_separators, unwrap_path_utf8},
    library::Library,
    style::Style,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum EpubSpineItemFormat {
    Svg,
    Xhtml,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubSpineItem {
    pub path: PathBuf,
    pub format: EpubSpineItemFormat,
    pub linear: bool,
    // properties can go here later once the rendering is complex enough to handle them, but ignore them for now
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubTocItem {
    label: String,
    path_without_fragment: PathBuf,
    path_with_fragment: PathBuf,
    children: Vec<EpubTocItem>,
    nesting_level: u64,
}

impl EpubTocItem {
    fn from_epub_library_representation(
        source: epub::doc::NavPoint,
        nesting_level: u64,
    ) -> anyhow::Result<Self> {
        let cleaned_path = standardize_path_separators(&source.content.clean());
        let mut path_split = unwrap_path_utf8(&cleaned_path)?
            .split('#')
            .collect::<Vec<_>>();
        let path_without_fragment = match path_split.len() {
            0 => PathBuf::new(), // This should be possible per the EPUB spec, even if the library is failing to expose it well.
            1 => PathBuf::from(
                path_split
                    .first()
                    .context("Unreachable: no first entry in vec of length 1.")?,
            ),
            _ => {
                let _fragment = path_split
                    .pop()
                    .context("Unreachable: no last entry in vec of length >1.")?;
                PathBuf::from(path_split.join("#"))
            }
        };

        let mut children = Vec::new();
        for source_child in source.children {
            children.push(Self::from_epub_library_representation(
                source_child,
                nesting_level + 1,
            )?);
        }

        Ok(Self {
            label: source.label,
            path_without_fragment,
            path_with_fragment: cleaned_path,
            children,
            nesting_level,
        })
    }

    fn flattened(&self) -> Vec<&Self> {
        self.children
            .iter()
            .fold(vec![self], |mut accumulator, child| {
                accumulator.append(&mut child.flattened());
                accumulator
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubRenditionInfo {
    pub style: Style,
    pub dir_path_from_library_root: PathBuf,
    pub default_file_path_from_library_root: PathBuf,
    pub bytes: u64,
}

impl EpubRenditionInfo {
    pub fn open_in_browser(
        &self,
        library_path: &Path,
        browser: &Option<String>,
    ) -> anyhow::Result<()> {
        let path_to_canonicalize = library_path.join(&self.default_file_path_from_library_root);
        let path_to_open = path_to_canonicalize
            .canonicalize() // To help cross-platform compatibility, hopefully
            .with_context(|| {
                format!(
                    "Unable to canonicalize book rendition path {} to open.",
                    path_to_canonicalize.display()
                )
            })?;
        browser::open(&path_to_open, browser)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EpubInfo {
    // Identifiers
    pub id: String,
    pub title: String,

    // Display-relevant metadata
    pub creators: Vec<String>,
    pub cover_path: Option<PathBuf>,
    pub first_linear_spine_item_path: PathBuf,
    pub last_linear_spine_item_path: PathBuf,
    // pub bodymatter_path: Option<PathBuf>,

    // Library-management-relevant metadata
    pub path_from_library_root: PathBuf,
    pub added_time: SystemTime,
    pub last_opened_time: SystemTime,
    pub last_opened_styles: Vec<Style>,

    // Contents
    pub spine_items: Vec<EpubSpineItem>,
    pub nonspine_resource_paths: Vec<PathBuf>,
    pub table_of_contents: Vec<EpubTocItem>,

    // Renditions
    pub raw_rendition: EpubRenditionInfo,
    pub nonraw_renditions: Vec<EpubRenditionInfo>, // Maybe change to hashset to improve worst-case perf?
}

impl EpubInfo {
    fn extract_epub_to_raw_dir(
        epub: &mut EpubDoc<BufReader<File>>,
        raw_dir_path: &Path,
    ) -> anyhow::Result<()> {
        for (id, resource) in epub.resources.clone() {
            let resource_path = raw_dir_path.join(resource.path);
            match resource_path.starts_with(raw_dir_path) {
                true => {
                    let resource_path_parent = resource_path
                        .parent()
                        .context("Unreachable: joined path is root.")?;
                    create_dir_all(resource_path_parent).with_context(|| {
                        format!(
                            "Failed to create directory {}.",
                            resource_path_parent.display()
                        )
                    })?;
                    let resource_bytes = epub.get_resource(&id).context("Internal error: EPUB library failed to get resource for id listed in its resources.")?.0;
                    write(&resource_path, resource_bytes).with_context(|| {
                        format!("Failed to write to {}.", resource_path.display())
                    })?;
                }
                false => bail!(
                    "Book contains resource {}, which is attempting a zip slip.",
                    resource_path.display()
                ),
            }
        }
        Ok(())
    }

    fn get_epub_creators(epub: &mut EpubDoc<BufReader<File>>) -> Vec<String> {
        epub.metadata
            .iter()
            .filter_map(|metadata_item| match &metadata_item.property == "creator" {
                true => Some(metadata_item.value.clone()),
                false => None,
            })
            .collect()
    }

    fn get_epub_cover_path(epub: &mut EpubDoc<BufReader<File>>) -> Option<PathBuf> {
        epub.get_cover_id().and_then(|cover_id| {
            epub.resources
                .get(&cover_id)
                .map(|cover_resource| standardize_path_separators(&cover_resource.path))
        })
    }

    fn get_epub_spine_items(
        epub: &mut EpubDoc<BufReader<File>>,
    ) -> anyhow::Result<Vec<EpubSpineItem>> {
        let mut spine_items = Vec::new();
        for item in &epub.spine {
            let item_resource = epub.resources.get(&item.idref).context(
                "Internal error: EPUB library failed to get resource for id listed in its spine.",
            )?;
            spine_items.push(EpubSpineItem {
                path: standardize_path_separators(&item_resource.path),
                format: match item_resource.mime.as_ref() {
                    "image/svg+xml" => EpubSpineItemFormat::Svg,
                    "application/xhtml+xml" => EpubSpineItemFormat::Xhtml,
                    other_mimetype => bail!("Ill-formed EPUB: encountered unexpected media type {other_mimetype} on spine item."),
                },
                linear: item.linear,
            });
        }
        Ok(spine_items)
    }

    fn get_epub_nonspine_resource_paths(
        epub: &mut EpubDoc<BufReader<File>>,
        spine_items: &[EpubSpineItem],
    ) -> Vec<PathBuf> {
        epub.resources
            .values()
            .filter_map(|resource| {
                let resource_path = &resource.path;
                match spine_items
                    .iter()
                    .any(|spine_item| &spine_item.path == resource_path)
                {
                    true => None,
                    false => Some(standardize_path_separators(resource_path)),
                }
            })
            .collect()
    }

    fn get_epub_table_of_contents(
        epub: &mut EpubDoc<BufReader<File>>,
    ) -> anyhow::Result<Vec<EpubTocItem>> {
        let mut toc = Vec::new();
        for navpoint in &epub.toc {
            toc.push(EpubTocItem::from_epub_library_representation(
                navpoint.clone(),
                0,
            )?);
        }
        Ok(toc)
    }

    pub fn new_from_epub(
        library: &mut Library,
        epub: &mut EpubDoc<BufReader<File>>,
        epub_id: String,
        request_time: SystemTime,
    ) -> anyhow::Result<Self> {
        let path_from_library_root = library.get_internal_path_from_id(&epub_id);
        let raw_dir_path_from_library_root = path_from_library_root.join("raw");
        let raw_dir_path = library.library_path.join(&raw_dir_path_from_library_root);

        Self::extract_epub_to_raw_dir(epub, &raw_dir_path)?;

        let creators = Self::get_epub_creators(epub);
        let cover_path = Self::get_epub_cover_path(epub);

        let spine_items = Self::get_epub_spine_items(epub)?;
        let nonspine_resource_paths = Self::get_epub_nonspine_resource_paths(epub, &spine_items);
        let table_of_contents = Self::get_epub_table_of_contents(epub)?;

        let first_linear_spine_item_path = standardize_path_separators(
            &spine_items
                .iter()
                .find(|item| item.linear)
                .context("Ill-formed EPUB: no linear spine items.")?
                .path,
        );
        let last_linear_spine_item_path = standardize_path_separators(
            &spine_items
                .iter()
                .rev()
                .find(|item| item.linear)
                .context("Ill-formed EPUB: no linear spine items.")?
                .path,
        );

        let raw_rendition_default_file_path_from_library_root =
            raw_dir_path_from_library_root.join(&first_linear_spine_item_path);

        Ok(Self {
            id: epub_id,
            title: epub.get_title().context("Ill-formed EPUB: no title.")?,
            creators,
            cover_path,
            first_linear_spine_item_path,
            last_linear_spine_item_path,
            path_from_library_root,
            added_time: request_time,
            last_opened_time: request_time,
            last_opened_styles: Vec::new(),
            spine_items,
            nonspine_resource_paths,
            table_of_contents,
            raw_rendition: EpubRenditionInfo {
                style: Style::raw(),
                default_file_path_from_library_root:
                    raw_rendition_default_file_path_from_library_root,
                dir_path_from_library_root: raw_dir_path_from_library_root,
                bytes: get_dir_size(&raw_dir_path)?,
            },
            nonraw_renditions: Vec::new(),
        })
    }

    pub fn find_rendition(&self, style: &Style) -> Option<&EpubRenditionInfo> {
        match style == &Style::raw() {
            true => Some(&self.raw_rendition),
            false => self
                .nonraw_renditions
                .iter()
                .find(|rendition| style == &rendition.style),
        }
    }

    pub fn size_in_bytes(&self) -> u64 {
        let nonraw_rendition_bytes = self
            .nonraw_renditions
            .iter()
            .fold(0, |bytes, rendition| bytes + rendition.bytes);
        self.raw_rendition.bytes + nonraw_rendition_bytes
    }

    pub fn get_new_rendition_dir_path_from_style(&self, style: &Style) -> PathBuf {
        static PADDING_AMOUNT: LazyLock<usize> = LazyLock::new(|| u64::MAX.to_string().len());

        let padded_style_hash = format!("{:0PADDING_AMOUNT$}", style.get_default_hash());
        let mut path_under_consideration = self.path_from_library_root.join(&padded_style_hash);
        let mut numeric_extension = 1;
        while self.nonraw_renditions.iter().any(|rendition_info| {
            rendition_info.dir_path_from_library_root == path_under_consideration
                && style != &rendition_info.style
        }) {
            numeric_extension += 1;
            path_under_consideration = self
                .path_from_library_root
                .join(format!("{padded_style_hash}_{numeric_extension}"));
        }

        path_under_consideration
    }
}
