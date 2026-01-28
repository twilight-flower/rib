pub mod consts;

use std::{
    fs::{read_dir, symlink_metadata},
    io::Write,
    path::Path,
};

use anyhow::{Context, bail};
use camino::{Utf8Path, Utf8PathBuf};
use url::Url;
use xml::{
    EventWriter,
    writer::{XmlEvent, events::StartElementBuilder},
};

use crate::{
    css::{CssBlock, CssBlockContents},
    style::Style,
};

////////////
//   fs   //
////////////

pub fn get_dir_size(path: &Path) -> anyhow::Result<u64> {
    // This doesn't follow symlinks; it only gets their size as symlink-text.
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

pub trait RibPathHelpers {
    fn standardize_separators(&self) -> Utf8PathBuf;
    fn to_file_url(&self) -> anyhow::Result<Url>;
    fn to_dir_url(&self) -> anyhow::Result<Url>;
}

impl RibPathHelpers for Utf8Path {
    fn standardize_separators(&self) -> Utf8PathBuf {
        self.components()
            .fold(Utf8PathBuf::new(), |mut pathbuf_out, component| {
                pathbuf_out.push(component);
                pathbuf_out
            })
    }

    fn to_file_url(&self) -> anyhow::Result<Url> {
        match Url::from_file_path(self) {
            Ok(url) => Ok(url),
            Err(()) => bail!("Internal error: couldn't convert path {self} to file URL."),
        }
    }

    fn to_dir_url(&self) -> anyhow::Result<Url> {
        match Url::from_directory_path(self) {
            Ok(url) => Ok(url),
            Err(()) => bail!("Internal error: couldn't convert path {self} to dir URL."),
        }
    }
}

///////////////
//   serde   //
///////////////

pub const fn return_true() -> bool {
    true
}

/////////////
//   url   //
/////////////

pub trait RibUrlHelpers {
    fn without_suffixes(&self) -> Self;
}

impl RibUrlHelpers for Url {
    fn without_suffixes(&self) -> Self {
        let mut suffixless_self = self.clone();
        suffixless_self.set_fragment(None);
        suffixless_self.set_query(None);
        suffixless_self
    }
}

/////////////
//   xml   //
/////////////

pub trait RibXmlWriterHelpers<W: Write> {
    fn write_xhtml_declaration(&mut self) -> anyhow::Result<()>;
    fn wrap_xml_element_write<F: FnOnce(&mut EventWriter<W>) -> anyhow::Result<()>>(
        &mut self,
        element_builder: StartElementBuilder,
        inner_write_fn: F,
    ) -> anyhow::Result<()>;
    fn write_xml_characters(&mut self, characters: &str) -> anyhow::Result<()>;
}

impl<W: Write> RibXmlWriterHelpers<W> for EventWriter<W> {
    fn write_xhtml_declaration(&mut self) -> anyhow::Result<()> {
        self.write(XmlEvent::StartDocument {
            version: xml::common::XmlVersion::Version10,
            encoding: Some("utf-8"),
            standalone: None,
        })
        .context("Failed to write XML document declaration.")?;
        self.inner_mut()
            .write(b"\n<!DOCTYPE html>")
            .context("Failed to write HTML doctype in XML context.")?;

        // `xml` library has crude doctype support, but it is newline-eating and therefore produces an uglier output for now. Hopefully this will change with time.
        // writer.write(XmlEvent::Doctype("<!DOCTYPE html>")).context("Failed to write HTML doctype in XML context.")?;

        Ok(())
    }

    fn wrap_xml_element_write<F: FnOnce(&mut EventWriter<W>) -> anyhow::Result<()>>(
        &mut self,
        element_builder: StartElementBuilder,
        inner_write_fn: F,
    ) -> anyhow::Result<()> {
        let element_event: XmlEvent = element_builder.into();
        let element_event_name = match element_event {
            XmlEvent::StartElement { name, .. } => name,
            _ => bail!("Unreachable: XML start element builder didn't build into start element."),
        };
        self.write(element_event).with_context(|| {
            format!("Failed to write {} XML element start.", element_event_name)
        })?;
        inner_write_fn(self)?;
        self.write(XmlEvent::EndElement {
            name: Some(element_event_name),
        })
        .with_context(|| format!("Failed to write {} XML element end.", element_event_name))?;
        Ok(())
    }

    fn write_xml_characters(&mut self, characters: &str) -> anyhow::Result<()> {
        self.write(XmlEvent::characters(characters))
            .context("Failed to write XML characters.")?;
        Ok(())
    }
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
    let mut block_contents = Vec::new();
    if let Some(height) = style.max_image_height() {
        block_contents.push(CssBlockContents::line(format!(
            "max-height: {};",
            height.value
        )));
    }
    if let Some(width) = style.max_image_width() {
        block_contents.push(CssBlockContents::line(format!(
            "max-width: {};",
            width.value
        )));
    }
    CssBlock::new("img", block_contents)
}

/////////////////
//   linking   //
/////////////////

pub fn create_link(source: &Utf8Path, destination: &Utf8Path) -> anyhow::Result<()> {
    // Unix-based systems use symlinks. Windows has permission issues with them, so uses hardlinks instead.
    #[cfg(windows)]
    std::fs::hard_link(destination, source)
        .with_context(|| format!("Failed to link {source} to {destination}."))?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(destination, source)
        .with_context(|| format!("Failed to link {source} to {destination}."))?;

    #[cfg(not(any(windows, unix)))]
    bail!("Unable to link {source} to {destination}: unsupported OS.");

    Ok(())
}
