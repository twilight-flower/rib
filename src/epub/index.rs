use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use itertools::Itertools;
use maud::{DOCTYPE, Markup, html};

use crate::{
    epub::{EpubInfo, EpubSpineItem, EpubTocItem},
    style::Style,
};

pub enum EpubIndex<'a> {
    TocLinearRelativeToSpine(Vec<(&'a EpubSpineItem, Vec<&'a EpubTocItem>)>),
    TocNonlinearRelativeToSpine(Vec<&'a EpubSpineItem>, Vec<&'a EpubTocItem>),
}

impl<'a> EpubIndex<'a> {
    fn flattened_toc_is_linear_relative_to_spine(
        spine: &[EpubSpineItem],
        flattened_toc: &[&EpubTocItem],
    ) -> anyhow::Result<bool> {
        // Make this more permissive about nonlinear spine-entries.
        let mut most_recent_spine_index = 0;
        for toc_item in flattened_toc {
            let toc_item_spine_index = spine
                .iter()
                .position(|spine_item| spine_item.path == toc_item.path_without_fragment)
                .ok_or(anyhow!(
                    "Invalid EPUB: TOC contains path {}, which doesn't appear in spine.",
                    toc_item.path_without_fragment.display()
                ))?;
            if toc_item_spine_index < most_recent_spine_index {
                return Ok(false);
            } else {
                most_recent_spine_index = toc_item_spine_index;
            }
        }
        Ok(true)
    }

    fn map_spine_to_flattened_toc<'b>(
        spine: &'a [EpubSpineItem],
        flattened_toc: &'b [&'a EpubTocItem],
    ) -> Vec<(&'a EpubSpineItem, Vec<&'a EpubTocItem>)> {
        // This currently doesn't handle paths with fragments. Fix.
        spine
            .iter()
            .map(|spine_item| {
                (
                    spine_item,
                    flattened_toc
                        .iter()
                        .filter(|toc_item| spine_item.path == toc_item.path_without_fragment)
                        .cloned()
                        .collect(),
                )
            })
            .collect()
    }

    pub fn from_spine_and_toc(
        spine: &'a [EpubSpineItem],
        toc: &'a [EpubTocItem],
    ) -> anyhow::Result<Self> {
        let flattened_toc = toc
            .iter()
            .map(|toc_item| toc_item.flattened())
            .flatten()
            .collect_vec();
        Ok(
            match Self::flattened_toc_is_linear_relative_to_spine(spine, &flattened_toc)? {
                true => Self::TocLinearRelativeToSpine(Self::map_spine_to_flattened_toc(
                    spine,
                    &flattened_toc,
                )),
                false => Self::TocNonlinearRelativeToSpine(spine.iter().collect(), flattened_toc),
            },
        )
    }

    fn list_toc_items_for_linear_index_spine_entry_recursive<
        T: Iterator<Item = &'a EpubTocItem>,
    >(
        spine_associated_toc_items_iter: &mut std::iter::Peekable<T>,
        rendition_contents_dir: &Path,
        current_ul_nesting_level: u64,
    ) -> Markup {
        // The while loop needs to be outside of the html macro because the html macro doesn't support break
        let mut html_fragment = html! {};
        while let Some(next_toc_item) = spine_associated_toc_items_iter.peek() {
            html_fragment = match current_ul_nesting_level.cmp(&next_toc_item.nesting_level) {
                Ordering::Less => html! {
                    (html_fragment)
                    ul {
                        (Self::list_toc_items_for_linear_index_spine_entry_recursive(spine_associated_toc_items_iter, rendition_contents_dir, current_ul_nesting_level + 1))
                    }
                },
                Ordering::Equal => {
                    let toc_item = spine_associated_toc_items_iter
                        .next()
                        .expect("Unreachable: no next item on peekable iter which peeked to Some.");
                    html! {
                        (html_fragment)
                        li {
                            a href=(rendition_contents_dir.join(&toc_item.path_with_fragment).display()) { (toc_item.label) }
                        }
                    }
                }
                Ordering::Greater => break, // This branch is currently untested; find a book to make sure it works
            }
        }
        html_fragment
    }

    fn list_toc_items_for_linear_index_spine_entry(
        spine_associated_toc_items: &[&'a EpubTocItem],
        rendition_contents_dir: &Path,
    ) -> Markup {
        Self::list_toc_items_for_linear_index_spine_entry_recursive(
            &mut spine_associated_toc_items.iter().copied().peekable(),
            rendition_contents_dir,
            0,
        )
    }

    fn list_toc_items_for_nonlinear_index(
        toc: &Vec<&EpubTocItem>,
        rendition_contents_dir: &Path,
    ) -> Markup {
        html! {
            @for toc_item in toc {
                li {
                    a href=(rendition_contents_dir.join(&toc_item.path_with_fragment).display()) { (toc_item.label) }
                }
                @if !toc_item.children.is_empty() {
                    ul {
                        (Self::list_toc_items_for_nonlinear_index(&toc_item.children.iter().collect(), rendition_contents_dir))
                    }
                }
            }
        }
    }

    pub fn to_html(&self, epub_info: &EpubInfo, style: Style) -> anyhow::Result<String> {
        // TODO: figure out if there's a more reliable way than .display() for stringifying pathbufs
        let rendition_contents_dir: PathBuf = match style.uses_raw_contents_dir() {
            true => ["..", "raw"].iter().collect(),
            false => "contents".into(),
        };
        Ok(html!(
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="utf-8";
                    title {
                        "rib | " (epub_info.title) " | Index"
                    }
                    link rel="stylesheet" href="index_styles.css"; // May need modding once userstyles are more a thing
                }
                body {
                    h1 { (epub_info.title) }
                    @if !epub_info.creators.is_empty() {
                        h3 { (epub_info.creators.join(" & ")) } // Fancify join logic later maybe?
                    }
                    @if let Some(cover_path) = &epub_info.cover_path {
                        img alt="book cover image" src=(rendition_contents_dir.join(cover_path).display());
                    }
                    p {
                        a href=(rendition_contents_dir.join(&epub_info.first_linear_spine_item_path).display()) { "Start" }
                    }
                    // Bodymatter link in similar style to start and end links, if there's a good way to get it within the limits of this epub crate
                    p {
                        a href=(rendition_contents_dir.join(&epub_info.last_linear_spine_item_path).display()) { "End" }
                    }
                    table {
                        @match self {
                            Self::TocLinearRelativeToSpine(mapping_vec) => {
                                tr {
                                    td { "Spine" }
                                    td { "Table of Contents" }
                                }
                                @for (spine_item, toc_items) in mapping_vec {
                                    tr {
                                        td {
                                            ul {
                                                li {
                                                    a href=(rendition_contents_dir.join(&spine_item.path).display()) { (spine_item.path.display()) }
                                                }
                                            }
                                        }
                                        td {
                                            @match toc_items.is_empty() {
                                                true => br;
                                                false => ul {
                                                    (Self::list_toc_items_for_linear_index_spine_entry(toc_items, &rendition_contents_dir))
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            Self::TocNonlinearRelativeToSpine(spine, toc) => {
                                tr {
                                    td { "Spine" }
                                    td { br; }
                                    td { "Table of Contents" }
                                }
                                tr {
                                    td {
                                        ul {
                                            @for spine_item in spine {
                                                // Maybe do something to mark nonlinear ones differently? (Previously they weren't rendered at all; this seemed suboptimal for usability.)
                                                li {
                                                    a href=(rendition_contents_dir.join(&spine_item.path).display()) { (spine_item.path.display()) }
                                                }
                                            }
                                        }
                                    }
                                    td { br; }
                                    td {
                                        ul {
                                            (Self::list_toc_items_for_nonlinear_index(toc, &rendition_contents_dir))
                                        }
                                    }
                                }
                            },
                        }
                    }
                }
            }
        )
        .into_string())
    }
}
