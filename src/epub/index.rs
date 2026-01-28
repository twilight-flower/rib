use std::{cmp::Ordering, io::Write};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use xml::{EventWriter, writer::XmlEvent};

use crate::{
    css::{CssBlock, CssBlockContents, CssFile},
    epub::{EpubInfo, EpubSpineItem, EpubTocItem},
    helpers::{
        RibXmlWriterHelpers, generate_stylesheet_img_block_unified,
        generate_stylesheet_link_block_unified,
    },
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
                .with_context(|| {
                    format!(
                        "Ill-formed EPUB: TOC contains path {}, which doesn't appear in spine.",
                        toc_item.path_without_fragment,
                    )
                })?;
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
            .flat_map(|toc_item| toc_item.flattened())
            .collect::<Vec<_>>();
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
        W: Write,
        T: Iterator<Item = &'a EpubTocItem>,
    >(
        writer: &mut EventWriter<W>,
        spine_associated_toc_items_iter: &mut std::iter::Peekable<T>,
        rendition_contents_dir: &Utf8Path,
        current_ul_nesting_level: u64,
    ) -> anyhow::Result<()> {
        while let Some(next_toc_item) = spine_associated_toc_items_iter.peek() {
            match current_ul_nesting_level.cmp(&next_toc_item.nesting_level) {
                Ordering::Less => {
                    writer.wrap_xml_element_write(XmlEvent::start_element("ul"), |writer| {
                        Self::list_toc_items_for_linear_index_spine_entry_recursive(
                            writer,
                            spine_associated_toc_items_iter,
                            rendition_contents_dir,
                            current_ul_nesting_level + 1,
                        )
                    })?
                }
                Ordering::Equal => {
                    let toc_item = spine_associated_toc_items_iter.next().context(
                        "Unreachable: no next item on peekable iter which peeked to Some.",
                    )?;
                    writer.wrap_xml_element_write(XmlEvent::start_element("li"), |writer| {
                        writer.wrap_xml_element_write(
                            XmlEvent::start_element("a").attr(
                                "href",
                                rendition_contents_dir
                                    .join(&toc_item.path_with_fragment)
                                    .as_str(),
                            ),
                            |writer| writer.write_xml_characters(&toc_item.label),
                        )
                    })?;
                }
                Ordering::Greater => break, // This branch is currently untested; find a book to make sure it works
            }
        }
        Ok(())
    }

    fn list_toc_items_for_linear_index_spine_entry<W: Write>(
        writer: &mut EventWriter<W>,
        spine_associated_toc_items: &[&'a EpubTocItem],
        rendition_contents_dir: &Utf8Path,
    ) -> anyhow::Result<()> {
        Self::list_toc_items_for_linear_index_spine_entry_recursive(
            writer,
            &mut spine_associated_toc_items.iter().copied().peekable(),
            rendition_contents_dir,
            0,
        )
    }

    fn list_toc_items_for_nonlinear_index<W: Write>(
        writer: &mut EventWriter<W>,
        toc: &Vec<&EpubTocItem>,
        rendition_contents_dir: &Utf8Path,
    ) -> anyhow::Result<()> {
        for toc_item in toc {
            writer.wrap_xml_element_write(XmlEvent::start_element("li"), |writer| {
                writer.wrap_xml_element_write(
                    XmlEvent::start_element("a").attr(
                        "href",
                        rendition_contents_dir
                            .join(&toc_item.path_with_fragment)
                            .as_str(),
                    ),
                    |writer| writer.write_xml_characters(&toc_item.label),
                )
            })?;
            if !toc_item.children.is_empty() {
                writer.wrap_xml_element_write(XmlEvent::start_element("ul"), |writer| {
                    Self::list_toc_items_for_nonlinear_index(
                        writer,
                        &toc_item.children.iter().collect(),
                        rendition_contents_dir,
                    )
                })?;
            }
        }
        Ok(())
    }

    pub fn to_xhtml(
        &self,
        epub_info: &EpubInfo,
        rendition_contents_dir_relative_path: Utf8PathBuf,
    ) -> anyhow::Result<Vec<u8>> {
        let xhtml_buffer = Vec::new();
        let mut writer = xml::EmitterConfig::new()
            .perform_indent(true)
            .indent_string("\t")
            .pad_self_closing(false)
            .create_writer(xhtml_buffer);

        writer.write_xhtml_declaration()?;
        writer.wrap_xml_element_write(
            XmlEvent::start_element("html")
                .default_ns("http://www.w3.org/1999/xhtml")
                .attr("lang", "en"),
            |writer| {
                writer.wrap_xml_element_write(XmlEvent::start_element("head"), |writer| {
                    writer.wrap_xml_element_write(
                        XmlEvent::start_element("meta").attr("charset", "utf-8"),
                        |_writer| Ok(()),
                    )?;
                    writer.wrap_xml_element_write(XmlEvent::start_element("title"), |writer| {
                        writer.write_xml_characters(&format!("rib | {} | Index", epub_info.title))
                    })?;
                    writer.wrap_xml_element_write(
                        XmlEvent::start_element("link")
                            .attr("rel", "stylesheet")
                            .attr("href", "index_styles.css"),
                        |_writer| Ok(()),
                    )?;
                    Ok(())
                })?;
                writer.wrap_xml_element_write(XmlEvent::start_element("body"), |writer| {
                    writer.wrap_xml_element_write(XmlEvent::start_element("h1"), |writer| {
                        writer.write_xml_characters(&epub_info.title)
                    })?;
                    if !epub_info.creators.is_empty() {
                        writer.wrap_xml_element_write(XmlEvent::start_element("h3"), |writer| {
                            // Fancify join logic later maybe?
                            writer.write_xml_characters(&epub_info.creators.join(" & "))
                        })?;
                    }
                    if let Some(cover_path) = &epub_info.cover_path {
                        writer.wrap_xml_element_write(
                            XmlEvent::start_element("img")
                                .attr("alt", "book cover image")
                                .attr(
                                    "src",
                                    rendition_contents_dir_relative_path
                                        .join(cover_path)
                                        .as_str(),
                                ),
                            |_writer| Ok(()),
                        )?;
                    }
                    writer.wrap_xml_element_write(XmlEvent::start_element("p"), |writer| {
                        writer.wrap_xml_element_write(
                            XmlEvent::start_element("a").attr(
                                "href",
                                rendition_contents_dir_relative_path
                                    .join(&epub_info.first_linear_spine_item_path)
                                    .as_str(),
                            ),
                            |writer| writer.write_xml_characters("Start"),
                        )
                    })?;
                    // Bodymatter link in similar style to start and end links, if there's a good way to get it within the limits of this epub crate
                    writer.wrap_xml_element_write(XmlEvent::start_element("p"), |writer| {
                        writer.wrap_xml_element_write(
                            XmlEvent::start_element("a").attr(
                                "href",
                                rendition_contents_dir_relative_path
                                    .join(&epub_info.last_linear_spine_item_path)
                                    .as_str(),
                            ),
                            |writer| writer.write_xml_characters("End"),
                        )
                    })?;
                    writer.wrap_xml_element_write(XmlEvent::start_element("table"), |writer| {
                        match self {
                            Self::TocLinearRelativeToSpine(mapping_vec) => {
                                writer.wrap_xml_element_write(
                                    XmlEvent::start_element("tr"),
                                    |writer| {
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| writer.write_xml_characters("Spine"),
                                        )?;
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.write_xml_characters("Table of Contents")
                                            },
                                        )?;
                                        Ok(())
                                    },
                                )?;
                                for (spine_item, toc_items) in mapping_vec {
                                    writer.wrap_xml_element_write(
                                        XmlEvent::start_element("tr"),
                                        |writer| {
                                            writer.wrap_xml_element_write(
                                                XmlEvent::start_element("td"),
                                                |writer| {
                                                    writer.wrap_xml_element_write(
                                                        XmlEvent::start_element("ul"),
                                                        |writer| {
                                                            writer.wrap_xml_element_write(
                                                                XmlEvent::start_element("li"),
                                                                |writer| {
                                                                    writer.wrap_xml_element_write(XmlEvent::start_element("a").attr("href", rendition_contents_dir_relative_path.join(&spine_item.path).as_str()), |writer| {
                                                                        writer.write_xml_characters(spine_item.path.as_str())
                                                                    })
                                                                },
                                                            )
                                                        },
                                                    )
                                                },
                                            )?;
                                            writer.wrap_xml_element_write(
                                                XmlEvent::start_element("td"),
                                                |writer| match toc_items.is_empty() {
                                                    true => writer.wrap_xml_element_write(
                                                        XmlEvent::start_element("br"),
                                                        |_writer| Ok(()),
                                                    ),
                                                    false => writer.wrap_xml_element_write(
                                                        XmlEvent::start_element("ul"),
                                                        |writer| {
                                                            Self::list_toc_items_for_linear_index_spine_entry(writer, toc_items, &rendition_contents_dir_relative_path)
                                                        },
                                                    ),
                                                },
                                            )?;
                                            Ok(())
                                        },
                                    )?;
                                }
                            }
                            Self::TocNonlinearRelativeToSpine(spine, toc) => {
                                writer.wrap_xml_element_write(
                                    XmlEvent::start_element("tr"),
                                    |writer| {
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| writer.write_xml_characters("Spine"),
                                        )?;
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.wrap_xml_element_write(
                                                    XmlEvent::start_element("br"),
                                                    |_writer| Ok(()),
                                                )
                                            },
                                        )?;
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.write_xml_characters("Table of Contents")
                                            },
                                        )?;
                                        Ok(())
                                    },
                                )?;
                                writer.wrap_xml_element_write(
                                    XmlEvent::start_element("tr"),
                                    |writer| {
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.wrap_xml_element_write(
                                                    XmlEvent::start_element("ul"),
                                                    |writer| {
                                                        for spine_item in spine {
                                                            writer.wrap_xml_element_write(
                                                                XmlEvent::start_element("li"),
                                                                |writer| {
                                                                    writer.wrap_xml_element_write(XmlEvent::start_element("a").attr("href", rendition_contents_dir_relative_path.join(&spine_item.path).as_str()), |writer| {
                                                                        writer.write_xml_characters(spine_item.path.as_str())
                                                                    })
                                                                },
                                                            )?;
                                                        }
                                                        Ok(())
                                                    },
                                                )
                                            },
                                        )?;
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.wrap_xml_element_write(
                                                    XmlEvent::start_element("br"),
                                                    |_writer| Ok(()),
                                                )
                                            },
                                        )?;
                                        writer.wrap_xml_element_write(
                                            XmlEvent::start_element("td"),
                                            |writer| {
                                                writer.wrap_xml_element_write(
                                                    XmlEvent::start_element("ul"),
                                                    |writer| {
                                                        Self::list_toc_items_for_nonlinear_index(
                                                            writer,
                                                            toc,
                                                            &rendition_contents_dir_relative_path,
                                                        )
                                                    },
                                                )
                                            },
                                        )?;
                                        Ok(())
                                    },
                                )?;
                            }
                        }
                        Ok(())
                    })?;
                    Ok(())
                })?;
                Ok(())
            },
        )?;

        Ok(writer.into_inner())
    }
}

fn generate_stylesheet_body_block(style: &Style) -> CssBlock {
    let mut block_contents = vec![CssBlockContents::line("text-align: center;")];

    if let Some(color) = style.text_color() {
        block_contents.push(CssBlockContents::line(format!("color: {};", color.value)));
    }
    if let Some(color) = style.background_color() {
        block_contents.push(CssBlockContents::line(format!(
            "background-color: {};",
            color.value
        )));
    }
    if let Some(margin) = style.margin_size() {
        block_contents.extend_from_slice(&[
            CssBlockContents::line(format!("margin-left: {};", margin.value)),
            CssBlockContents::line(format!("margin-right: {};", margin.value)),
        ]);
    }

    CssBlock::new("body", block_contents)
}

fn generate_stylesheet_td_block(style: &Style) -> CssBlock {
    let border_color = match style.text_color() {
        Some(color) => &color.value,
        None => "black",
    };
    CssBlock::new(
        "td",
        vec![
            CssBlockContents::line(format!("border: 1px solid {border_color};")),
            CssBlockContents::line("vertical-align: top;"),
        ],
    )
}

pub fn generate_stylesheet(style: &Style) -> anyhow::Result<String> {
    CssFile::new(vec![
        generate_stylesheet_body_block(style),
        CssBlock::new(
            "table",
            vec![
                CssBlockContents::line("border-collapse: collapse;"),
                CssBlockContents::line("margin-left: auto;"),
                CssBlockContents::line("margin-right: auto;"),
            ],
        ),
        generate_stylesheet_td_block(style),
        CssBlock::new("ul", vec![CssBlockContents::line("text-align: left;")]),
        generate_stylesheet_link_block_unified(style),
        generate_stylesheet_img_block_unified(style),
    ])
    .to_string()
    .context("Internal error: failed to generate index stylesheet.")
}
