use std::{fs::read, io::Cursor, ops::IndexMut, str::FromStr};

use anyhow::Context;
use camino::Utf8Path;
use pathdiff::diff_utf8_paths;
use url::Url;
use xml::{EmitterConfig, reader::XmlEvent};

use crate::{
    css::{CssBlock, CssBlockContents, CssFile},
    epub::{EpubInfo, navigation::create_navigation_wrapper},
    helpers::{consts::XHTML_ENTITIES, wrap_xml_element_write},
    style::Style,
};

fn generate_stylesheet_body_block(style: &Style, override_book: bool) -> CssBlock {
    let (selector, importance) = match override_book {
        true => ("body", " !important"),
        false => (":where(body)", ""),
    };
    let mut block_contents = Vec::new();

    if let Some(color) = style.text_color()
        && color.override_book == override_book
    {
        block_contents.push(CssBlockContents::line(format!(
            "color: {}{importance};",
            color.value
        )));
    }
    if let Some(color) = style.background_color()
        && color.override_book == override_book
    {
        block_contents.push(CssBlockContents::line(format!(
            "background-color: {}{importance};",
            color.value
        )));
    }
    if let Some(margin) = style.margin_size()
        && margin.override_book == override_book
    {
        block_contents.extend_from_slice(&[
            CssBlockContents::line(format!("margin-left: {}{importance};", margin.value)),
            CssBlockContents::line(format!("margin-right: {}{importance};", margin.value)),
        ]);
    }

    CssBlock::new(selector, block_contents)
}

fn generate_stylesheet_link_block(style: &Style, override_book: bool) -> CssBlock {
    match style.link_color() {
        Some(color) if color.override_book == override_book => match override_book {
            true => CssBlock::new(
                ":any-link",
                vec![CssBlockContents::line(format!(
                    "color: {} !important;",
                    color.value
                ))],
            ),
            false => CssBlock::new(
                ":where(:any-link)",
                vec![CssBlockContents::line(format!("color: {};", color.value))],
            ),
        },
        _ => CssBlock::empty(),
    }
}

fn generate_stylesheet_img_block(style: &Style, override_book: bool) -> CssBlock {
    let block_prefix = match override_book {
        true => "img",
        false => ":where(img)",
    };
    let mut block_contents = Vec::new();
    if let Some(height) = style.max_image_height()
        && height.override_book == override_book
    {
        match override_book {
            true => block_contents.push(CssBlockContents::line(format!(
                "max-height: {} !important;",
                height.value
            ))),
            false => block_contents.push(CssBlockContents::line(format!(
                "max-height: {};",
                height.value
            ))),
        }
    }
    if let Some(width) = style.max_image_width()
        && width.override_book == override_book
    {
        match override_book {
            true => block_contents.push(CssBlockContents::line(format!(
                "max-width: {} !important;",
                width.value
            ))),
            false => block_contents.push(CssBlockContents::line(format!(
                "max-width: {};",
                width.value
            ))),
        }
    }
    CssBlock::new(block_prefix, block_contents)
}

pub fn generate_stylesheets(style: &Style) -> (Option<String>, Option<String>) {
    let no_override_sheet = CssFile::new(vec![
        generate_stylesheet_body_block(style, false),
        generate_stylesheet_link_block(style, false),
        generate_stylesheet_img_block(style, false),
    ]);
    let override_sheet = CssFile::new(vec![
        generate_stylesheet_body_block(style, true),
        generate_stylesheet_link_block(style, true),
        generate_stylesheet_img_block(style, true),
    ]);
    (no_override_sheet.to_string(), override_sheet.to_string())
}

fn adjust_xhtml_source(
    source_path: &Utf8Path,
    destination_path: &Utf8Path,
    no_override_stylesheet_path: Option<&Utf8Path>,
    override_stylesheet_path: Option<&Utf8Path>,
    style: &Style,
) -> anyhow::Result<Vec<u8>> {
    let source_file =
        read(source_path).with_context(|| format!("Failed to read {source_path}."))?;
    let reader = xml::ParserConfig::new()
        .add_entities(XHTML_ENTITIES)
        .ignore_comments(false)
        .override_encoding(Some(xml::Encoding::Utf8))
        .create_reader(Cursor::new(source_file));

    let adjusted_source_buffer = Vec::new();
    let mut adjusted_source_buffer_writer = EmitterConfig::new()
        .write_document_declaration(false)
        .normalize_empty_elements(false)
        .autopad_comments(false)
        .pad_self_closing(false)
        .create_writer(adjusted_source_buffer);

    let relative_path_target = match style.inject_navigation {
        true => "_parent",
        false => "_self",
    };

    let destination_path_parent = destination_path
        .parent()
        .context("Internal error: attempted to adjust XHTML with root as its destination path.")?;
    // Note: we use `destination_path_parent`, not `destination_path`, as base for relative stylesheet-links, because `diff_paths` assumes all its paths are dirs rather than files and so adds an extra `..` component relative to the path-logic that XHTML operates under.

    for event in reader {
        match event.context("XML parse failure.")? {
            XmlEvent::StartElement {
                name,
                mut attributes,
                namespace,
            } if name.local_name == "a"
                && name
                    .namespace
                    .as_ref()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml") =>
            {
                // Rewrite <a> elements to open in appropriate locations: current tab if relative, new tab if absolute
                let href_value = attributes.iter().find_map(|attribute| {
                    match attribute.name.local_name == "href" {
                        true => Some(&attribute.value),
                        false => None,
                    }
                });
                let target = match href_value {
                    Some(value) => Some(match Url::parse(value) {
                        Ok(_absolute_path) => "_blank".to_string(),
                        Err(url::ParseError::RelativeUrlWithoutBase) => {
                            relative_path_target.to_string()
                        }
                        Err(e) => {
                            return Err(e).with_context(|| {
                                format!("URL parse error on <a href=\"{value}\">")
                            });
                        }
                    }),
                    None => None,
                };
                if let Some(target_unwrapped) = target {
                    match attributes
                        .iter()
                        .position(|attribute| attribute.name.local_name == "target")
                    {
                        Some(target_attribute_index) => {
                            attributes.index_mut(target_attribute_index).value = target_unwrapped;
                        }
                        None => {
                            attributes.push(xml::attribute::OwnedAttribute {
                                name: xml::name::OwnedName::from_str("target").ok().context(
                                    "Failed to add \"target\" attribute to <a> element.",
                                )?,
                                value: target_unwrapped,
                            });
                        }
                    }
                }
                let reader_event_rebuilt = XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                };
                let writer_event = reader_event_rebuilt.as_writer_event().context(
                    "Internal error: failed to convert reader <a> StartElement event to writer format.",
                )?;
                adjusted_source_buffer_writer
                    .write(writer_event)
                    .context("Failed to write updated <a> element XML to new buffer.")?;
            }
            XmlEvent::StartElement {
                name,
                attributes,
                namespace,
            } if name.local_name == "head"
                && name
                    .namespace
                    .as_ref()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml")
                && no_override_stylesheet_path.is_some() =>
            {
                // Inject no-override styles at start of head if they exist
                let stylesheet_path_absolute = no_override_stylesheet_path.context(
                    "Unreachable: no-override stylesheet path is Some but can't be unwrapped.",
                )?;
                let stylesheet_path_relative =
                    diff_utf8_paths(stylesheet_path_absolute, destination_path_parent).with_context(
                        || {
                            format!(
                                "Internal error: failed to generate path from {destination_path_parent} to {stylesheet_path_absolute}."
                            )
                        },
                    )?;

                let reader_event_rebuilt = XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                };
                let writer_event = reader_event_rebuilt.as_writer_event().context(
                    "Internal error: failed to convert reader <head> StartElement event to writer format.",
                )?;
                adjusted_source_buffer_writer
                    .write(writer_event)
                    .context("Failed to write <head> element XML to new buffer.")?;
                wrap_xml_element_write(
                    &mut adjusted_source_buffer_writer,
                    xml::writer::events::XmlEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", stylesheet_path_relative.as_str()),
                    |_writer| Ok(()),
                )?;
            }
            XmlEvent::EndElement { name }
                if name.local_name == "head"
                    && name
                        .namespace
                        .as_ref()
                        .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml")
                    && override_stylesheet_path.is_some() =>
            {
                // Inject override styles at end of head if they exist
                let stylesheet_path_absolute = override_stylesheet_path.context(
                    "Unreachable: override stylesheet path is Some but can't be unwrapped.",
                )?;
                let stylesheet_path_relative =
                    diff_utf8_paths(stylesheet_path_absolute, destination_path_parent).with_context(
                        || {
                            format!(
                                "Internal error: failed to generate path from {destination_path_parent} to {stylesheet_path_absolute}."
                            )
                        },
                    )?;

                wrap_xml_element_write(
                    &mut adjusted_source_buffer_writer,
                    xml::writer::events::XmlEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", stylesheet_path_relative.as_str()),
                    |_writer| Ok(()),
                )?;
                let reader_event_rebuilt = XmlEvent::EndElement { name };
                let writer_event = reader_event_rebuilt.as_writer_event().context(
                    "Internal error: failed to convert reader </head> EndElement event to writer format.",
                )?;
                adjusted_source_buffer_writer
                    .write(writer_event)
                    .context("Failed to write </head> element XML to new buffer.")?;
            }
            other_reader_event => {
                // For otherwise-unmarked reader events, transcribe them unchanged
                if let Some(writer_event) = other_reader_event.as_writer_event() {
                    adjusted_source_buffer_writer
                        .write(writer_event)
                        .context("Failed to write parsed XML to new buffer.")?;
                }
            }
        }
    }

    Ok(adjusted_source_buffer_writer.into_inner())
}

#[allow(clippy::too_many_arguments)]
pub fn adjust_spine_xhtml(
    epub_info: &EpubInfo,
    contents_dir_path: &Utf8Path,
    source_path: &Utf8Path,
    destination_path: &Utf8Path,
    no_override_stylesheet_path: Option<&Utf8Path>,
    override_stylesheet_path: Option<&Utf8Path>,
    spine_index: usize,
    style: &Style,
) -> anyhow::Result<Vec<u8>> {
    let adjusted_source = adjust_xhtml_source(
        source_path,
        destination_path,
        no_override_stylesheet_path,
        override_stylesheet_path,
        style,
    );
    match style.inject_navigation {
        true => {
            let adjusted_source_string =
                String::from_utf8(adjusted_source?).with_context(|| {
                    format!(
                        "Internal error: {source_path} wasn't encoded to valid UTF-8 on adjustment."
                    )
                })?;
            create_navigation_wrapper(
                epub_info,
                contents_dir_path,
                destination_path,
                spine_index,
                style,
                &adjusted_source_string,
            )
        }
        false => adjusted_source,
    }
}
