use anyhow::Context;
use camino::Utf8Path;
use xml::{EmitterConfig, writer::XmlEvent};

use crate::{
    css::{CssBlock, CssBlockContents, CssFile},
    epub::{EpubInfo, SpineNavigationMap},
    helpers::{
        RibXmlWriterHelpers, generate_stylesheet_img_block_unified,
        generate_stylesheet_link_block_unified,
    },
    style::Style,
};

fn get_previous_linear_spine_item_path<'a>(
    spine_navigation_maps: &'a [SpineNavigationMap],
    current_spine_index: usize,
) -> anyhow::Result<&'a str> {
    // Assumption: previous linear spine item path exists.
    let mut next_index_to_check = current_spine_index - 1;
    loop {
        match spine_navigation_maps.get(next_index_to_check) {
            Some(SpineNavigationMap { spine_item, navigation_filename }) if spine_item.linear => return Ok(navigation_filename),
            Some(_) => next_index_to_check -= 1,
            None => return None.context("Internal error: called get_previous_linear_spine_item_path when no previous linear spine item path could be gotten."),
        }
    }
}

fn get_next_linear_spine_item_path<'a>(
    spine_navigation_maps: &'a [SpineNavigationMap],
    current_spine_index: usize,
) -> anyhow::Result<&'a str> {
    // Assumption: next linear spine item path exists.
    let mut next_index_to_check = current_spine_index + 1;
    loop {
        match spine_navigation_maps.get(next_index_to_check) {
            Some(SpineNavigationMap { spine_item, navigation_filename }) if spine_item.linear => return Ok(navigation_filename),
            Some(_) => next_index_to_check += 1,
            None => return None.context("Internal error: called get_next_linear_spine_item_path when no next linear spine item path could be gotten."),
        }
    }
}

pub fn create_navigation_wrapper(
    epub_info: &EpubInfo,
    spine_navigation_maps: &[SpineNavigationMap],
    spine_index: usize,
    style: &Style,
    section_path: &Utf8Path,
) -> anyhow::Result<Vec<u8>> {
    let navigation_wrapper_buffer = Vec::new();
    let mut navigation_wrapper_buffer_writer = EmitterConfig::new()
        .perform_indent(true)
        .indent_string("\t")
        .normalize_empty_elements(false) // Needs to be false to avoid problems when page is parsed as non-X HTML due to non-`.xhtml` filename
        .create_writer(navigation_wrapper_buffer);

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

    navigation_wrapper_buffer_writer.write_xhtml_declaration()?;
    navigation_wrapper_buffer_writer.wrap_xml_element_write(
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
                    // Maybe add section name here where the index has " | Index", if I can think of a good way to generate those?
                    writer.write_xml_characters(&format!("rib | {}", epub_info.title))
                })?;
                writer.wrap_xml_element_write(
                    XmlEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("href", "navigation_styles.css"),
                    |_writer| Ok(()),
                )?;
                Ok(())
            })?;
            writer.wrap_xml_element_write(XmlEvent::start_element("body"), |writer| {
                writer.wrap_xml_element_write(
                    // It'd be nice to do sandboxing here to block scripts until I've considered whether I want to allow them, but hard to configure in a way that doesn't break things. So leave it out for now.
                    XmlEvent::start_element("iframe")
                        .attr("id", "section")
                        .attr("src", section_path.as_str()),
                    |_writer| Ok(()),
                )?;
                writer.wrap_xml_element_write(
                    XmlEvent::start_element("nav").attr("id", "navigation"),
                    |writer| {
                        // Currently there's no dropdown navigation menu, just an index button. Consider changing this later.
                        match spine_index <= first_linear_section_index {
                            true => writer.wrap_xml_element_write(
                                XmlEvent::start_element("a").attr("class", "navigation-button"),
                                |writer| writer.write_xml_characters("Previous"),
                            ),
                            false => {
                                let previous_linear_spine_item_path =
                                    get_previous_linear_spine_item_path(
                                        spine_navigation_maps,
                                        spine_index,
                                    )?;
                                writer.wrap_xml_element_write(
                                    XmlEvent::start_element("a")
                                        .attr("class", "navigation-button")
                                        .attr("href", previous_linear_spine_item_path),
                                    |writer| writer.write_xml_characters("Previous"),
                                )
                            }
                        }?;
                        if style.include_index {
                            writer.wrap_xml_element_write(
                                XmlEvent::start_element("a")
                                    .attr("class", "navigation-button")
                                    .attr("href", "index.xhtml"),
                                |writer| writer.write_xml_characters("Index"),
                            )?;
                        }
                        match spine_index >= last_linear_section_index {
                            true => writer.wrap_xml_element_write(
                                XmlEvent::start_element("button")
                                    .attr("type", "button")
                                    .attr("disabled", "disabled"),
                                |writer| writer.write_xml_characters("Next"),
                            ),
                            false => {
                                let next_linear_spine_item_path = get_next_linear_spine_item_path(
                                    spine_navigation_maps,
                                    spine_index,
                                )?;
                                writer.wrap_xml_element_write(
                                    XmlEvent::start_element("a")
                                        .attr("class", "navigation-button")
                                        .attr("href", next_linear_spine_item_path),
                                    |writer| writer.write_xml_characters("Next"),
                                )
                            }
                        }?;
                        Ok(())
                    },
                )?;
                writer.wrap_xml_element_write(
                    XmlEvent::start_element("script").attr("src", "navigation_script.js"),
                    |_writer| Ok(()),
                )?;
                Ok(())
            })?;
            Ok(())
        },
    )?;

    Ok(navigation_wrapper_buffer_writer.into_inner())
}

fn generate_stylesheet_body_block(style: &Style) -> CssBlock {
    let mut block_contents = vec![
        CssBlockContents::line("margin: 0;"),
        CssBlockContents::line("padding: 0;"),
        CssBlockContents::line("height: 100vh;"),
        CssBlockContents::line("width: 100vw;"),
        CssBlockContents::line("overflow: hidden;"),
    ];

    if let Some(color) = style.text_color() {
        block_contents.push(CssBlockContents::line(format!("color: {};", color.value)));
    }
    if let Some(color) = style.background_color() {
        block_contents.push(CssBlockContents::line(format!(
            "background-color: {};",
            color.value
        )));
    }

    CssBlock::new("body", block_contents)
}

fn generate_stylesheet_navigation_block(style: &Style) -> CssBlock {
    let (left_and_right_position, width) = match style.margin_size() {
        Some(margin) => (
            format!("calc(5vh + {})", margin.value),
            format!(
                "calc(100vw - calc(10vh + 2.5rem + calc(2 * {})))",
                margin.value
            ),
        ),
        None => (
            "5vh".to_string(),
            "calc(100vw - calc(10vh + 2.5rem))".to_string(),
        ),
    };

    let border_color = match style.text_color() {
        Some(color) => &color.value,
        None => "black",
    };

    let background_color = match style.background_color() {
        Some(color) => &color.value,
        None => "white",
    };

    CssBlock::new(
        "#navigation",
        vec![
            // Position
            // Maybe drop specifying right position? It's possibly redundant
            CssBlockContents::line("position: fixed;"),
            CssBlockContents::line("bottom: 5vh;"),
            CssBlockContents::line(format!("left: {left_and_right_position};")),
            CssBlockContents::line(format!("right: {left_and_right_position};")),
            CssBlockContents::line(format!("width: {width};")),
            // Style
            CssBlockContents::line("padding: 1rem;"),
            CssBlockContents::line(format!("border: 0.25rem solid {border_color};")),
            CssBlockContents::line("border-radius: 2rem;"),
            CssBlockContents::line(format!("background: {background_color};")),
            // Contents style
            CssBlockContents::line("text-align: center;"),
            // Hide when not in use
            CssBlockContents::line("opacity: 0;"),
            CssBlockContents::line("transition: opacity 0.4s ease-out;"),
        ],
    )
}

fn generate_stylesheet_navigation_button_block(style: &Style) -> CssBlock {
    let border_color = match style.text_color() {
        Some(color) => &color.value,
        None => "black",
    };
    CssBlock::new(
        ".navigation-button",
        vec![
            CssBlockContents::line("padding: 0.1rem;"),
            CssBlockContents::line(format!("border: 0.1rem solid {border_color};")),
            CssBlockContents::line("border-radius: 0.2rem;"),
            CssBlockContents::line("text-decoration: none;"),
            // Maybe also make the text color consistent? Dunno; TBD
        ],
    )
}

pub fn generate_stylesheet(style: &Style) -> anyhow::Result<String> {
    CssFile::new(vec![
        generate_stylesheet_body_block(style),
        CssBlock::new(
            "#section",
            vec![
                CssBlockContents::line("border: none;"),
                CssBlockContents::line("height: 100%;"),
                CssBlockContents::line("width: 100%;"),
            ],
        ),
        generate_stylesheet_navigation_block(style),
        CssBlock::new(
            "#navigation:hover",
            vec![CssBlockContents::line("opacity: 1;")],
        ),
        generate_stylesheet_navigation_button_block(style),
        generate_stylesheet_link_block_unified(style),
        generate_stylesheet_img_block_unified(style),
    ])
    .to_string()
    .context("Internal error: failed to generate navigation stylesheet.")
}
