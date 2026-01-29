use std::{fs::read, io::Cursor, ops::IndexMut, str::FromStr};

use anyhow::Context;
use camino::Utf8Path;
use url::{ParseError, Url};
use xml::{EmitterConfig, reader::XmlEvent as XmlReaderEvent, writer::XmlEvent as XmlWriterEvent};

use crate::{
    css::{CssBlock, CssBlockContents, CssFile},
    epub::SpineNavigationMap,
    helpers::{RibPathHelpers, RibUrlHelpers, RibXmlWriterHelpers, consts::XHTML_ENTITIES},
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

pub fn adjust_xhtml_source(
    contents_dir_path: &Utf8Path,
    source_path: &Utf8Path,
    destination_path: &Utf8Path,
    no_override_stylesheet_path: Option<&Utf8Path>,
    override_stylesheet_path: Option<&Utf8Path>,
    spine_navigation_maps: &[SpineNavigationMap],
    style: &Style,
) -> anyhow::Result<Vec<u8>> {
    let source = read(source_path).with_context(|| format!("Failed to read {source_path}."))?;
    let reader = xml::ParserConfig::new()
        .add_entities(XHTML_ENTITIES)
        .ignore_comments(false)
        .override_encoding(Some(xml::Encoding::Utf8))
        .create_reader(Cursor::new(source));

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

    let contents_dir_path_parent = contents_dir_path
        .parent()
        .context("Internal error: contents dir was root.")?;

    let contents_dir_url = contents_dir_path.to_dir_url()?;
    let destination_url = destination_path.to_file_url()?;

    for event in reader {
        match event.context("XML parse failure.")? {
            XmlReaderEvent::StartElement {
                name,
                mut attributes,
                namespace,
            } if name.local_name == "a"
                && name
                    .namespace
                    .as_ref()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml") =>
            {
                // Rewrite <a> elements to two effects.
                // First, set their targets to appropriate locations: current tab if relative, new tab if absolute
                // Second, if injecting navigation, have relative hrefs open the target page's associated navigation-page rather than the target page itself
                let href = attributes
                    .iter_mut()
                    .find(|attribute| attribute.name.local_name == "href");
                let target = match href {
                    Some(ref href_attribute) => Some(match Url::parse(&href_attribute.value) {
                        Ok(_absolute_path) => "_blank".to_string(),
                        Err(ParseError::RelativeUrlWithoutBase) => relative_path_target.to_string(),
                        Err(e) => {
                            return Err(e).with_context(|| {
                                format!(
                                    r#"URL parse error on <a href="{}">"#,
                                    &href_attribute.value
                                )
                            });
                        }
                    }),
                    None => None,
                };
                if let Some(attribute) = href
                    && style.inject_navigation
                {
                    let mut href_url_absolute =
                        destination_url.join(&attribute.value).with_context(|| {
                            format!(
                                "Couldn't parse {} as URL relative to {destination_url}",
                                &attribute.value
                            )
                        })?;
                    if let Some(href_url_from_contents_dir) =
                        contents_dir_url.make_relative(&href_url_absolute.without_suffixes())
                        && let Some(spine_navigation_map) =
                            spine_navigation_maps.iter().find(|spine_navigation_map| {
                                spine_navigation_map.spine_item.path == href_url_from_contents_dir
                            })
                    {
                        href_url_absolute.set_path(
                            contents_dir_path_parent
                                .join(&spine_navigation_map.navigation_filename)
                                .as_str(),
                        );
                        let url_redirected_to_navigation = destination_url.make_relative(&href_url_absolute).with_context(|| format!("Internal error: failed to get relative URL from {destination_url} to {href_url_absolute}."))?;
                        attribute.value = url_redirected_to_navigation;
                    }
                }
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
                let reader_event_rebuilt = XmlReaderEvent::StartElement {
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
            XmlReaderEvent::StartElement {
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
                let stylesheet_url_absolute = no_override_stylesheet_path
                    .context(
                        "Unreachable: no-override stylesheet path is Some but can't be unwrapped.",
                    )?
                    .to_file_url()?;
                let stylesheet_url_relative = destination_url.make_relative(&stylesheet_url_absolute).with_context(|| format!("Internal error: failed to get relative URL from {destination_url} to {stylesheet_url_absolute}."))?;

                let reader_event_rebuilt = XmlReaderEvent::StartElement {
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
                adjusted_source_buffer_writer.wrap_xml_element_write(
                    XmlWriterEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", &stylesheet_url_relative),
                    |_writer| Ok(()),
                )?;
            }
            XmlReaderEvent::EndElement { name }
                if name.local_name == "head"
                    && name
                        .namespace
                        .as_ref()
                        .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml")
                    && override_stylesheet_path.is_some() =>
            {
                // Inject override styles at end of head if they exist
                let stylesheet_url_absolute = override_stylesheet_path
                    .context(
                        "Unreachable: override stylesheet path is Some but can't be unwrapped.",
                    )?
                    .to_file_url()?;
                let stylesheet_url_relative = destination_url.make_relative(&stylesheet_url_absolute).with_context(|| format!("Internal error: failed to get relative URL from {destination_url} to {stylesheet_url_absolute}."))?;

                adjusted_source_buffer_writer.wrap_xml_element_write(
                    XmlWriterEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", stylesheet_url_relative.as_str()),
                    |_writer| Ok(()),
                )?;
                let reader_event_rebuilt = XmlReaderEvent::EndElement { name };
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

pub fn wrap_xhtml_source_for_navigation(
    rendition_dir_path: &Utf8Path,
    source_path_from_rendition_dir: &Utf8Path,
) -> anyhow::Result<String> {
    let source_path_absolute = rendition_dir_path.join(source_path_from_rendition_dir);
    let source = read(&source_path_absolute)
        .with_context(|| format!("Failed to read {source_path_absolute}."))?;

    let first_pass_reader = xml::ParserConfig::new()
        .add_entities(XHTML_ENTITIES)
        .ignore_comments(false)
        .override_encoding(Some(xml::Encoding::Utf8))
        .create_reader(Cursor::new(&source));

    let mut base_href = None;

    // First pass: read and determine the correct href for the base attribute to be written
    for event in first_pass_reader {
        match event.context("XML parse failure.")? {
            XmlReaderEvent::StartElement {
                name,
                attributes,
                namespace,
            } if name.local_name == "base"
                && name
                    .namespace
                    .as_ref()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml") =>
            {
                // Determine our base's correct href based on the one used here, if one is specified here
                if let Some(href_attribute) = attributes
                    .iter()
                    .find(|attribute| attribute.name.local_name == "href")
                {
                    match Url::parse(&href_attribute.value) {
                        Ok(_) => {
                            // If the href is to an absolute URL, it remains the correct base
                            base_href = Some(href_attribute.value.clone());
                            break;
                        }
                        Err(ParseError::RelativeUrlWithoutBase) => {
                            // If the href is to a relative URL, prefix it with the source file path from the navigation file
                            let base_url = rendition_dir_path.to_dir_url()?;
                            let url_with_navigation_path = base_url.join(source_path_from_rendition_dir.as_str()).with_context(|| format!("Internal error: couldn't join {source_path_from_rendition_dir} to {base_url} during wrapping-for-navigation of source XHTML."))?;
                            let url_with_href_joined = url_with_navigation_path.join(&href_attribute.value).with_context(|| format!("Ill-formed EPUB: couldn't join base {} to {url_with_navigation_path}.", &href_attribute.value))?;
                            let new_href_url = base_url.make_relative(&url_with_href_joined).with_context(|| format!("Ill-formed EPUB: couldn't get relative URL from {base_url} to {url_with_href_joined}."))?;
                            base_href = Some(new_href_url);
                            break;
                        }
                        Err(e) => {
                            return Err(e).with_context(|| {
                                format!(
                                    r#"URL parse error on <base href="{}">"#,
                                    &href_attribute.value
                                )
                            });
                        }
                    }
                }
            }
            XmlReaderEvent::EndElement { name }
                if name.local_name == "head"
                    && name
                        .namespace
                        .as_ref()
                        .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml") =>
            {
                // If no base href was found in the head before its end, use the source path from the navigation file
                base_href = Some(source_path_from_rendition_dir.as_str().to_string());
                break;
            }
            _ => (),
        }
    }

    let base_href_unwrapped =
        base_href.context("Ill-formed EPUB: XHTML content document has no head ending tag.")?;

    let second_pass_reader = xml::ParserConfig::new()
        .add_entities(XHTML_ENTITIES)
        .ignore_comments(false)
        .override_encoding(Some(xml::Encoding::Utf8))
        .create_reader(Cursor::new(source));

    let wrapped_source_buffer = Vec::new();
    let mut wrapped_source_buffer_writer = EmitterConfig::new()
        .write_document_declaration(false)
        .normalize_empty_elements(false)
        .autopad_comments(false)
        .pad_self_closing(false)
        .create_writer(wrapped_source_buffer);

    // Second pass: write base with defined href to head, superseding any other base hrefs which might be defined later
    for event in second_pass_reader {
        match event.context("XML parse failure.")? {
            XmlReaderEvent::StartElement {
                name,
                attributes,
                namespace,
            } if name.local_name == "head"
                && name
                    .namespace
                    .as_ref()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/1999/xhtml") =>
            {
                // Write start of head, then immediately write base tag with the specified href
                let reader_event_rebuilt = XmlReaderEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                };
                let writer_event = reader_event_rebuilt.as_writer_event().context(
                    "Internal error: failed to convert reader <head> StartElement event to writer format.",
                )?;
                wrapped_source_buffer_writer
                    .write(writer_event)
                    .context("Failed to write <head> element XML to new buffer.")?;

                wrapped_source_buffer_writer.wrap_xml_element_write(
                    XmlWriterEvent::start_element("base").attr("href", &base_href_unwrapped),
                    |_writer| Ok(()),
                )?;
            }
            other_reader_event => {
                // Transcribe everything else unchanged
                if let Some(writer_event) = other_reader_event.as_writer_event() {
                    wrapped_source_buffer_writer
                        .write(writer_event)
                        .context("Failed to write parsed XML to new buffer.")?;
                }
            }
        }
    }

    let wrapped_source_string = String::from_utf8(wrapped_source_buffer_writer.into_inner()).with_context(|| format!("Internal error: {source_path_absolute} wasn't encoded to valid UTF-8 after wrapping for navigation."))?;
    Ok(wrapped_source_string)
}
