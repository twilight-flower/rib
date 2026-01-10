use std::{fs::read, io::Cursor, ops::IndexMut, path::Path, str::FromStr};

use anyhow::Context;
use pathdiff::diff_paths;
use url::Url;
use xml::{EmitterConfig, writer::XmlEvent};

use crate::{
    epub::{EpubInfo, EpubSpineItem},
    helpers::{
        unwrap_path_utf8, wrap_xml_element_write, write_xhtml_declaration, write_xml_characters,
    },
    style::Style,
};

fn adjust_xhtml_source(source_path: &Path, style: &Style) -> anyhow::Result<Vec<u8>> {
    let source_file =
        read(source_path).with_context(|| format!("Failed to read {}.", source_path.display()))?;
    let reader = xml::ParserConfig::new()
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

    for event in reader {
        match event.context("XML parse failure.")? {
            xml::reader::XmlEvent::StartElement {
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
                let reader_event_rebuilt = xml::reader::XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                };
                let writer_event = reader_event_rebuilt.as_writer_event().context(
                    "Internal error: failed to convert reader StartElement event to writer format.",
                )?;
                adjusted_source_buffer_writer
                    .write(writer_event)
                    .context("Failed to write updated <a> element XML to new buffer.")?;
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

fn get_previous_linear_spine_item_path(
    epub_info: &EpubInfo,
    current_spine_index: usize,
) -> anyhow::Result<&Path> {
    // Assumption: previous linear spine item path exists.
    let mut next_index_to_check = current_spine_index - 1;
    loop {
        match epub_info.spine_items.get(next_index_to_check) {
            Some(EpubSpineItem { path, linear }) if *linear => return Ok(path),
            Some(_) => next_index_to_check -= 1,
            None => return None.context("Internal error: called get_previous_linear_spine_item_path when no previous linear spine item path could be gotten."),
        }
    }
}

fn get_next_linear_spine_item_path(
    epub_info: &EpubInfo,
    current_spine_index: usize,
) -> anyhow::Result<&Path> {
    // Assumption: next linear spine item path exists.
    let mut next_index_to_check = current_spine_index + 1;
    loop {
        match epub_info.spine_items.get(next_index_to_check) {
            Some(EpubSpineItem { path, linear }) if *linear => return Ok(path),
            Some(_) => next_index_to_check += 1,
            None => return None.context("Internal error: called get_next_linear_spine_item_path when no next linear spine item path could be gotten."),
        }
    }
}

fn create_navigation_wrapper(
    epub_info: &EpubInfo,
    contents_dir_path: &Path,
    destination_path: &Path,
    spine_index: usize,
    style: &Style,
    source: &str,
) -> anyhow::Result<Vec<u8>> {
    // This might be sensible to factor into a `navigation` module once SVG support exists too
    let navigation_wrapper_buffer = Vec::new();
    let mut navigation_wrapper_buffer_writer = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("\t")
        .pad_self_closing(false)
        .create_writer(navigation_wrapper_buffer);

    let contents_dir_path_parent = contents_dir_path
        .parent()
        .context("Internal error: rendition contents dir is root.")?;
    let destination_path_parent = destination_path.parent().context(
        "Internal error: attempted to create navigation wrapper with root as its destination path.",
    )?;
    // Note: we use `destination_path_parent`, not `destination_path`, as base for relative links out of the XHTML, because `diff_paths` assumes all its paths are dirs rather than files and so adds an extra `..` component relative to the path-logic that XHTML operates under.

    let stylesheet_path_absolute = contents_dir_path_parent.join("navigation_styles.css");
    let stylesheet_path_relative = diff_paths(&stylesheet_path_absolute, destination_path_parent)
        .with_context(|| {
        format!(
            "Internal error: failed to generate path from {} to {}.",
            destination_path_parent.display(),
            stylesheet_path_absolute.display()
        )
    })?;

    let first_linear_section_index = epub_info
        .spine_items
        .iter()
        .position(|item| item.path == epub_info.first_linear_spine_item_path)
        .context("Ill-formed EPUB: no linear spine items.")?;
    let last_linear_section_index = epub_info
        .spine_items
        .iter()
        .position(|item| item.path == epub_info.last_linear_spine_item_path)
        .context("Ill-formed EPUB: no linear spine items.")?;

    write_xhtml_declaration(&mut navigation_wrapper_buffer_writer)?;
    wrap_xml_element_write(
        &mut navigation_wrapper_buffer_writer,
        XmlEvent::start_element("html")
            .default_ns("http://www.w3.org/1999/xhtml")
            .attr("lang", "en"),
        |writer| {
            wrap_xml_element_write(writer, XmlEvent::start_element("head"), |writer| {
                wrap_xml_element_write(
                    writer,
                    XmlEvent::start_element("meta").attr("charset", "utf-8"),
                    |_writer| Ok(()),
                )?;
                wrap_xml_element_write(writer, XmlEvent::start_element("title"), |writer| {
                    // Maybe add section name here where the index has " | Index", if I can think of a good way to generate those?
                    write_xml_characters(writer, &format!("rib | {}", epub_info.title))
                })?;
                wrap_xml_element_write(
                    // May need modding once userstyles are more a thing
                    writer,
                    XmlEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", unwrap_path_utf8(&stylesheet_path_relative)?),
                    |_writer| Ok(()),
                )?;
                Ok(())
            })?;
            wrap_xml_element_write(writer, XmlEvent::start_element("body"), |writer| {
                wrap_xml_element_write(
                    writer,
                    XmlEvent::start_element("iframe")
                        .attr("id", "section")
                        .attr("srcdoc", source),
                    |_writer| Ok(()),
                )?;
                wrap_xml_element_write(
                    writer,
                    XmlEvent::start_element("nav").attr("id", "navigation"),
                    |writer| {
                        // Currently there's no dropdown navigation menu, just an index button. Consider changing this later.
                        match spine_index <= first_linear_section_index {
                            true => wrap_xml_element_write(
                                writer,
                                XmlEvent::start_element("a").attr("class", "navigation-button"),
                                |writer| write_xml_characters(writer, "Previous"),
                            ),
                            false => {
                                let previous_linear_spine_item_path =
                                    get_previous_linear_spine_item_path(epub_info, spine_index)?;
                                let previous_linear_spine_item_path_absolute =
                                    contents_dir_path.join(&previous_linear_spine_item_path);
                                let previous_linear_spine_item_path_relative = diff_paths(
                                    &previous_linear_spine_item_path_absolute,
                                    destination_path_parent,
                                )
                                .with_context(|| {
                                    format!(
                                        "Internal error: failed to generate path from {} to {}.",
                                        destination_path_parent.display(),
                                        stylesheet_path_absolute.display()
                                    )
                                })?;
                                wrap_xml_element_write(
                                    writer,
                                    XmlEvent::start_element("a")
                                        .attr("class", "navigation-button")
                                        .attr(
                                            "href",
                                            unwrap_path_utf8(
                                                &previous_linear_spine_item_path_relative,
                                            )?,
                                        ),
                                    |writer| write_xml_characters(writer, "Previous"),
                                )
                            }
                        }?;
                        if style.include_index {
                            let index_path_absolute = contents_dir_path_parent.join("index.xhtml");
                            let index_path_relative = diff_paths(
                                &index_path_absolute,
                                &destination_path_parent,
                            )
                            .with_context(|| {
                                format!(
                                    "Internal error: failed to generate path from {} to {}.",
                                    destination_path_parent.display(),
                                    index_path_absolute.display()
                                )
                            })?;
                            wrap_xml_element_write(
                                writer,
                                XmlEvent::start_element("a")
                                    .attr("class", "navigation-button")
                                    .attr("href", unwrap_path_utf8(&index_path_relative)?),
                                |writer| write_xml_characters(writer, "Index"),
                            )?;
                        }
                        match spine_index >= last_linear_section_index {
                            true => wrap_xml_element_write(
                                writer,
                                XmlEvent::start_element("button")
                                    .attr("type", "button")
                                    .attr("disabled", "disabled"),
                                |writer| write_xml_characters(writer, "Next"),
                            ),
                            false => {
                                let next_linear_spine_item_path =
                                    get_next_linear_spine_item_path(epub_info, spine_index)?;
                                let next_linear_spine_item_path_absolute =
                                    contents_dir_path.join(&next_linear_spine_item_path);
                                let next_linear_spine_item_path_relative = diff_paths(
                                    &next_linear_spine_item_path_absolute,
                                    destination_path_parent,
                                )
                                .with_context(|| {
                                    format!(
                                        "Internal error: failed to generate path from {} to {}.",
                                        destination_path_parent.display(),
                                        stylesheet_path_absolute.display()
                                    )
                                })?;
                                wrap_xml_element_write(
                                    writer,
                                    XmlEvent::start_element("a")
                                        .attr("class", "navigation-button")
                                        .attr(
                                            "href",
                                            unwrap_path_utf8(
                                                &next_linear_spine_item_path_relative,
                                            )?,
                                        ),
                                    |writer| write_xml_characters(writer, "Next"),
                                )
                            }
                        }?;
                        Ok(())
                    },
                )?;
                Ok(())
            })?;
            Ok(())
        },
    )?;

    Ok(navigation_wrapper_buffer_writer.into_inner())
}

pub fn adjust_spine_xhtml(
    epub_info: &EpubInfo,
    contents_dir_path: &Path,
    source_path: &Path,
    destination_path: &Path,
    spine_index: usize,
    style: &Style,
) -> anyhow::Result<Vec<u8>> {
    let adjusted_source = adjust_xhtml_source(source_path, style);
    match style.inject_navigation {
        true => {
            let adjusted_source_string =
                String::from_utf8(adjusted_source?).with_context(|| {
                    format!(
                        "Internal error: {} wasn't encoded to valid UTF-8 on adjustment.",
                        source_path.display()
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
