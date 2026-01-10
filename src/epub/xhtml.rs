use std::{fs::read, io::Cursor, ops::IndexMut, path::Path, str::FromStr};

use anyhow::Context;
use url::Url;
use xml::EmitterConfig;

use crate::{
    epub::{EpubInfo, navigation::create_navigation_wrapper},
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
