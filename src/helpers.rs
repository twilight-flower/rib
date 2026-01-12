use std::{
    fs::{hard_link, read_dir, symlink_metadata},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use xml::{
    EventWriter,
    writer::{XmlEvent, events::StartElementBuilder},
};

#[cfg(not(any(windows, unix)))]
use anyhow::anyhow;

use crate::{
    css::{CssBlock, CssBlockContents},
    style::Style,
};

////////////
//   fs   //
////////////

pub fn get_dir_size(path: &Path) -> anyhow::Result<u64> {
    // Doesn't follow symlinks. Could be pretty easily modded to do so if useful later.
    read_dir(path)
        .with_context(|| format!("Couldn't read {} as directory.", path.display()))?
        .try_fold(0, |bytes, maybe_dir_entry| {
            let dir_entry = maybe_dir_entry
                .with_context(|| format!("Error viewing entry in {}", path.display()))?;
            let dir_entry_path = dir_entry.path();
            let dir_entry_metadata = symlink_metadata(&dir_entry_path).with_context(|| {
                format!("Couldn't read metadata for {}.", dir_entry_path.display())
            })?;
            Ok(bytes
                + match dir_entry_metadata.is_dir() {
                    true => get_dir_size(&dir_entry_path)?,
                    false => dir_entry_metadata.len(),
                })
        })
}

//////////////
//   path   //
//////////////

pub fn standardize_path_separators(pathbuf_in: &Path) -> PathBuf {
    pathbuf_in
        .components()
        .fold(PathBuf::new(), |mut pathbuf_out, component| {
            pathbuf_out.push(component);
            pathbuf_out
        })
}

pub fn unwrap_path_utf8(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .context("Ill-formed EPUB: non-UTF-8 path encountered.")
}

///////////////
//   serde   //
///////////////

pub const fn return_true() -> bool {
    true
}

/////////////
//   xml   //
/////////////

pub fn write_xhtml_declaration<W: Write>(writer: &mut EventWriter<W>) -> anyhow::Result<()> {
    writer
        .write(XmlEvent::StartDocument {
            version: xml::common::XmlVersion::Version10,
            encoding: Some("utf-8"),
            standalone: None,
        })
        .context("Failed to write XML document declaration.")?;
    writer
        .inner_mut()
        .write(b"\n<!DOCTYPE html>")
        .context("Failed to write HTML doctype in XML context.")?;

    // `xml` library has crude doctype support, but it is newline-eating and therefore produces an uglier output for now. Hopefully this will change with time.
    // writer.write(XmlEvent::Doctype("<!DOCTYPE html>")).context("Failed to write HTML doctype in XML context.")?;

    Ok(())
}

pub fn wrap_xml_element_write<W: Write, F: FnOnce(&mut EventWriter<W>) -> anyhow::Result<()>>(
    writer: &mut EventWriter<W>,
    element_builder: StartElementBuilder,
    inner_write_fn: F,
) -> anyhow::Result<()> {
    let element_event: XmlEvent = element_builder.into();
    let element_event_name = match element_event {
        XmlEvent::StartElement { name, .. } => name,
        _ => bail!("Unreachable: XML start element builder didn't build into start element."),
    };
    writer
        .write(element_event)
        .with_context(|| format!("Failed to write {} XML element start.", element_event_name))?;
    inner_write_fn(writer)?;
    writer
        .write(XmlEvent::EndElement {
            name: Some(element_event_name),
        })
        .with_context(|| format!("Failed to write {} XML element end.", element_event_name))?;
    Ok(())
}

pub fn write_xml_characters<W: Write>(
    writer: &mut EventWriter<W>,
    characters: &str,
) -> anyhow::Result<()> {
    writer
        .write(XmlEvent::characters(characters))
        .context("Failed to write XML characters.")?;
    Ok(())
}

/////////////
//   css   //
/////////////

pub fn generate_stylesheet_link_block_unified(style: &Style) -> CssBlock {
    let block_contents = match style.link_color() {
        Some(color) => vec![CssBlockContents::line(format!("color: {};", color.value))],
        None => Vec::new(),
    };
    CssBlock::new(":any-link", block_contents)
}

pub fn generate_stylesheet_img_block_unified(style: &Style) -> CssBlock {
    let block_contents = match style.max_image_width() {
        Some(width) => vec![CssBlockContents::line(format!(
            "max-width: {};",
            width.value
        ))],
        None => Vec::new(),
    };
    CssBlock::new("img", block_contents)
}

/////////////////
//   linking   //
/////////////////

pub fn create_link(source: &Path, destination: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    hard_link(destination, source).with_context(|| {
        format!(
            "Failed to link {} to {}.",
            source.display(),
            destination.display()
        )
    })?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(destination, source);

    #[cfg(not(any(windows, unix)))]
    anyhow!(
        "Unable to link {} to {}: unsupported OS.",
        source.display(),
        destination.display()
    );

    Ok(())
}
