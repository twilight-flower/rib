use std::path::Path;

use anyhow::Context;
use pathdiff::diff_paths;
use xml::{EmitterConfig, writer::XmlEvent};

use crate::{
    epub::{EpubInfo, EpubSpineItem},
    helpers::{
        unwrap_path_utf8, wrap_xml_element_write, write_xhtml_declaration, write_xml_characters,
    },
    style::Style,
};

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

pub fn create_navigation_wrapper(
    epub_info: &EpubInfo,
    contents_dir_path: &Path,
    destination_path: &Path,
    spine_index: usize,
    style: &Style,
    source: &str,
) -> anyhow::Result<Vec<u8>> {
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

pub fn generate_stylesheet(_style: &Style) -> String {
    r#"
body {
	margin: 0;
	padding: 0;
	height: 100vh;
	width: 100vw;
	overflow: hidden;
}

#section {
	border: none;
	height: 100%;
	width: 100%;
}

#navigation {
	position: fixed;
	bottom: 5vh;
	left: 5vh;
	right: 5vh;
	width: calc(100vw - calc(10vh + 2.5rem));

	padding: 1rem;
	border: 0.25rem solid black;
	border-radius: 2rem;
	background: white;

	text-align: center;

	opacity: 0;
	transition: opacity 0.4s ease-out;
}

#navigation:hover {
	opacity: 1;
}

.navigation-button {
	padding: 0.1rem;
	border: 0.1rem solid black;
	border-radius: 0.2rem;
	text-decoration: none;
	/* Maybe also make the text color consistent? Dunno; TBD */
}
"#
    .trim_start()
    .to_string()
}
