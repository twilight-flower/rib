use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::fs::{File, create_dir_all, read_to_string, write};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Command;

use argh::FromArgs;
use directories::ProjectDirs;
use epub::doc::EpubDoc;
use maud::{DOCTYPE, html};
use serde::{Deserialize, Serialize};
use quick_xml::events::{BytesText, Event};

//////////////
//   Args   //
//////////////

#[derive(Clone, Debug, FromArgs)]
/// Minimalist EPUB reader.
struct Args {
    #[argh(positional)]
    /// epub path to open
    epub: String,
    #[argh(option, short = 'b')]
    /// browser to open output with
    browser: Option<String>,
    #[argh(switch, short = 'B')]
    /// don't open output in browser
    browser_skip: bool,
    #[argh(option, short = 's')]
    /// stylesheet name (in config.toml) to apply to output
    stylesheet: Option<String>,
    // To add: single-book overrides for individual styles
}

////////////////
//   Config   //
////////////////

#[derive(Clone, Debug, Deserialize)]
struct StyleFont {
    value: String,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleFontSize {
    value: i64,
    override_book: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct StyleTextColor {
    value: String,
    override_book: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct StyleLinkColor {
    value: String,
    override_book: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct StyleBackgroundColor {
    value: String,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleLineSpacing {
    value: f64,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleIndentation {
    value: i64,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleMarginSize {
    value: i64,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleMaxWidth {
    value: i64,
    override_book: bool,
}

#[derive(Copy, Clone, Debug, Deserialize)]
struct StyleLimitImageSizeToViewportSize {
    value: bool,
    override_book: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct Stylesheet {
    font: Option<StyleFont>,
    font_size: Option<StyleFontSize>,
    text_color: Option<StyleTextColor>,
    link_color: Option<StyleLinkColor>,
    background_color: Option<StyleBackgroundColor>,
    line_spacing: Option<StyleLineSpacing>,
    indentation: Option<StyleIndentation>,
    margin_size: Option<StyleMarginSize>,
    max_width: Option<StyleMaxWidth>,
    limit_image_size_to_viewport_size: Option<StyleLimitImageSizeToViewportSize>,
    freeform_css_no_override: Option<String>,
    freeform_css_override: Option<String>,
}

impl Stylesheet {
    fn empty() -> Self {
        Self {
            font: None,
            font_size: None,
            text_color: None,
            link_color: None,
            background_color: None,
            line_spacing: None,
            indentation: None,
            margin_size: None,
            max_width: None,
            limit_image_size_to_viewport_size: None,
            freeform_css_no_override: None,
            freeform_css_override: None,
        }
    }

    fn has_no_override_styles (&self) -> bool {
        (self.font.is_some() && !self.font.as_ref().unwrap().override_book)
        || (self.font_size.is_some() && !self.font_size.as_ref().unwrap().override_book)
        || (self.text_color.is_some() && !self.text_color.as_ref().unwrap().override_book)
        || (self.link_color.is_some() && !self.link_color.as_ref().unwrap().override_book)
        || (self.background_color.is_some() && !self.background_color.as_ref().unwrap().override_book)
        || (self.line_spacing.is_some() && !self.line_spacing.as_ref().unwrap().override_book)
        || (self.indentation.is_some() && !self.indentation.as_ref().unwrap().override_book)
        || (self.margin_size.is_some() && !self.margin_size.as_ref().unwrap().override_book)
        || (self.max_width.is_some() && !self.margin_size.as_ref().unwrap().override_book)
        || (self.limit_image_size_to_viewport_size.is_some() && !self.limit_image_size_to_viewport_size.as_ref().unwrap().override_book)
        || self.freeform_css_no_override.is_some()
    }

    fn has_override_styles (&self) -> bool {
        (self.font.is_some() && self.font.as_ref().unwrap().override_book)
        || (self.font_size.is_some() && self.font_size.as_ref().unwrap().override_book)
        || (self.text_color.is_some() && self.text_color.as_ref().unwrap().override_book)
        || (self.link_color.is_some() && self.link_color.as_ref().unwrap().override_book)
        || (self.background_color.is_some() && self.background_color.as_ref().unwrap().override_book)
        || (self.line_spacing.is_some() && self.line_spacing.as_ref().unwrap().override_book)
        || (self.indentation.is_some() && self.indentation.as_ref().unwrap().override_book)
        || (self.margin_size.is_some() && self.margin_size.as_ref().unwrap().override_book)
        || (self.max_width.is_some() && self.margin_size.as_ref().unwrap().override_book)
        || (self.limit_image_size_to_viewport_size.is_some() && self.limit_image_size_to_viewport_size.as_ref().unwrap().override_book)
        || self.freeform_css_override.is_some()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Config {
    default_browser: String,
    max_cache_books: usize,
    max_cache_bytes: usize,
    default_stylesheet: String,
    stylesheets: HashMap<String, Stylesheet>,
}

impl Config {
    fn open(path: &PathBuf) -> Self {
        match read_to_string(path) {
            Ok(file) => toml::from_str(&file).expect(&format!("Config file is invalid or incorrectly-structured TOML.")),
            Err(_) => {
                println!("No preexisting config file found. Attempting to create new config file with default settings at {}.", path.display());
                create_dir_all(path.parent().unwrap()).expect("Failed to create config dir.");
                write(path, include_str!("default_config.toml")).expect("Failed to create config file.");
                Self::open(path)
            }
        }
    }
}

///////////////
//   Cache   //
///////////////

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CachedBook {
    id: String,
    path: PathBuf,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct Cache {
    path: PathBuf,
    contents: VecDeque<CachedBook>,
    max_books: Option<usize>,
    max_bytes: Option<usize>,
}

impl Cache {
    fn open(path: PathBuf, config: &Config) -> Self {
        let contents = match read_to_string(&path) {
            Ok(file) => serde_json::from_str(&file).expect("Cache index is invalid or incorrectly-structured JSON."),
            Err(_) => {
                create_dir_all(path.parent().unwrap()).expect("Failed to create cache dir.");
                write(&path, "[]").expect("Failed to create cache index.");
                VecDeque::new()
            }
        };
        Self {
            path,
            contents,
            max_books: match config.max_cache_books {
                0 => None,
                _ => Some(config.max_cache_books),
            },
            max_bytes: match config.max_cache_bytes {
                0 => None,
                _ => Some(config.max_cache_bytes),
            }
        }
    }

    fn write(&self) {
        let contents_serialized = serde_json::to_string_pretty(&self.contents).unwrap();
        write(&self.path, contents_serialized).expect("Failed to update cache index.");
    }

    fn count_books(&self) -> usize {
        self.contents.len()
    }

    fn count_bytes(&self) -> usize {
        self.contents.iter().map(|book| book.bytes).sum()
    }

    fn remove_oldest(&mut self) {
        self.contents.pop_front().expect("Called remove_oldest on empty cache.");
        self.write();
    }

    fn add(&mut self, id: String, dirname: String, bytes: usize) {
        if self.max_books.is_some() {
            while self.count_books() >= self.max_books.unwrap() {
                self.remove_oldest();
            }
        }
        if self.max_bytes.is_some() {
            while self.count_books() > 0 && (self.count_bytes() + bytes) > self.max_bytes.unwrap() {
                self.remove_oldest();
            }
        }

        self.contents.push_back(CachedBook {
            id,
            path: self.path.join(dirname),
            bytes,
        });
        
        self.write();
    }
}

/////////////////////////////
//   Miscellaneous Types   //
/////////////////////////////

#[derive(Clone, Debug)]
struct TocItem {
    iri: PathBuf,
    path: PathBuf,
    label: String,
    children: Vec<TocItem>,
    nesting_level: usize,
}

#[derive(Clone, Debug)]
struct SpineItem {
    path: PathBuf,
    linear: bool,
}

///////////////////
//   Functions   //
///////////////////

fn write_navigation_element(writer: &mut quick_xml::Writer<Vec<u8>>, book_contents_dir: &PathBuf, book_index_path: &PathBuf, spine: &Vec<SpineItem>, spine_position: usize) {
    // This currently doesn't work if the spine items have '.xhtml' extensions, because apparently browser recognition of XHTML versus HTML is down to file extension. Figure out a fix, probably involving format-conversion.
    use quick_xml::Error;

    let previous_spine_path = if spine_position > 0 {
        Some(&spine[spine_position - 1].path)
    } else {
        None
    };
    let next_spine_path = if spine_position < (spine.len() - 1) {
        Some(&spine[spine_position + 1].path)
    } else {
        None
    };

    writer.create_element("div").write_inner_content::<_, Error>(|writer| {
        writer.create_element("template").with_attribute(("shadowrootmode", "closed")).write_inner_content::<_, Error>(|writer| {
            writer.create_element("nav").with_attribute(("style", "text-align: center;")).write_inner_content::<_, Error>(|writer| {
                // Previous button
                match previous_spine_path {
                    Some(path) => writer.create_element("a").with_attribute(("href", book_contents_dir.join(path).as_os_str().to_str().unwrap())).write_inner_content::<_, Error>(|writer| {
                        writer.create_element("button").with_attribute(("type", "button")).write_text_content(BytesText::new("Previous")).expect("XHTML writing error.");
                        Ok(())
                    }).expect("XHTML writing error."),
                    None => writer.create_element("button").with_attributes([("type", "button"), ("disabled", "disabled")]).write_text_content(BytesText::new("Previous")).expect("XHTML writing error."),
                };
                // Index button
                writer.create_element("a").with_attribute(("href", book_index_path.as_os_str().to_str().unwrap())).write_inner_content::<_, Error>(|writer| {
                    writer.create_element("button").with_attribute(("type", "button")).write_text_content(BytesText::new("Index")).expect("XHTML writing error.");
                    Ok(())
                }).expect("XHTML writing error.");
                // Next button
                match next_spine_path {
                    Some(path) => writer.create_element("a").with_attribute(("href", book_contents_dir.join(path).as_os_str().to_str().unwrap())).write_inner_content::<_, Error>(|writer| {
                        writer.create_element("button").with_attribute(("type", "button")).write_text_content(BytesText::new("Next")).expect("XHTML writing error.");
                        Ok(())
                    }).expect("XHTML writing error."),
                    None => writer.create_element("button").with_attributes([("type", "button"), ("disabled", "disabled")]).write_text_content(BytesText::new("Next")).expect("XHTML writing error."),
                };
                Ok(())
            }).expect("XHTML writing error.");
            Ok(())
        }).expect("XHTML writing error.");
        Ok(())
    }).expect("XHTML writing error.");
}

fn inject_navigation(xhtml: &Vec<u8>, book_contents_dir: &PathBuf, book_index_path: &PathBuf, spine: &Vec<SpineItem>, spine_position: usize) -> Vec<u8> {
    let mut reader = quick_xml::Reader::from_reader(xhtml.as_ref());
    let reader_config = reader.config_mut();
    reader_config.enable_all_checks(true);
    reader_config.expand_empty_elements = true;
    let mut writer = quick_xml::Writer::new(Vec::new());

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"body" => {
                writer.write_event(Event::Start(e)).expect("XHTML writing error.");
                write_navigation_element(&mut writer, book_contents_dir, book_index_path, spine, spine_position);
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"body" => {
                write_navigation_element(&mut writer, book_contents_dir, book_index_path, spine, spine_position);
                writer.write_event(Event::End(e)).expect("XHTML writing error.");
            }
            Ok(Event::Eof) => break,
            Ok(e) => writer.write_event(e.borrow()).expect("XHTML writing error."),
            Err(e) => Err(e).expect("XHTML reading error."),
        }
    }

    writer.into_inner()
}

fn inject_styles(xhtml: &Vec<u8>, stylesheet: &Stylesheet, css_path: &PathBuf) -> (Vec<u8>, Option<Vec<u8>>) {
    (xhtml.clone(), None) // Updated HTML, new stylesheet if applicable; placeholder
}

fn process_spine_xhtml(xhtml: &Vec<u8>, book_contents_dir: &PathBuf, book_index_path: &PathBuf, spine: &Vec<SpineItem>, spine_position: usize, stylesheet: &Stylesheet, css_path: &PathBuf) -> (Vec<u8>, Option<Vec<u8>>) {
    let xhtml_with_navigation = inject_navigation(&xhtml, book_contents_dir, book_index_path, spine, spine_position);
    inject_styles(&xhtml_with_navigation, stylesheet, css_path)
}

fn create_index_css(stylesheet: &Stylesheet) -> Option<String> {
    // Add non-inline styling for the index table (overriding even override styles, at least pending addition of an override-even-reader-UI style-category)
    let mut css = String::new();

    let mut body_styles = Vec::new();
    if let Some(font) = &stylesheet.font {
        body_styles.push(format!("font-family: {};", font.value));
    }
    if let Some(font_size) = &stylesheet.font_size {
        body_styles.push(format!("font-size: {}px;", font_size.value));
    }
    if let Some(text_color) = &stylesheet.text_color {
        body_styles.push(format!("color: {};", text_color.value));
    }
    if let Some(background_color) = &stylesheet.background_color {
        body_styles.push(format!("background: {};", background_color.value));
    }
    if let Some(line_spacing) = &stylesheet.line_spacing {
        body_styles.push(format!("line-height: {};", line_spacing.value));
    }
    if let Some(indentation) = &stylesheet.indentation {
        body_styles.push(format!("text-indent: {}px;", indentation.value));
    }
    if let Some(max_width) = &stylesheet.max_width {
        // Confirm this actually works
        body_styles.push(format!("max-width: {}px;", max_width.value));
    }
    if let Some(margin_size) = &stylesheet.margin_size {
        body_styles.push(format!("margin-left: {}px; margin-right: {}px;", margin_size.value, margin_size.value));
    }

    if !body_styles.is_empty() {
        css = format!("body {{{}}}\n", body_styles.join(" "))
    }
    if let Some(link_color) = &stylesheet.link_color {
        css = format!("{}{}", css, format!("a {{color: {}}}\n", link_color.value));
    }
    if let Some(StyleLimitImageSizeToViewportSize {
        value: true,
        ..
    }) = &stylesheet.limit_image_size_to_viewport_size {
        // Make sure this plays well with margins
        css = format!("{}img {{max-width: 100%}};\n", css);
    }
    if let Some(freeform_css_no_override) = &stylesheet.freeform_css_no_override {
        css = format!("{}{}\n", css, freeform_css_no_override);
    }
    if let Some(freeform_css_override) = &stylesheet.freeform_css_override {
        css = format!("{}{}\n", css, freeform_css_override);
    }

    if css.is_empty() {
        None
    } else {
        Some(css)
    }
}

fn localize_toc_item_format(nav_point: epub::doc::NavPoint, nesting_level: usize) -> TocItem {

    let mut path_split = nav_point.content.to_str().unwrap().split("#").collect::<Vec<&str>>();
    let path = match path_split.len() {
        0 => PathBuf::new(), // This should be possible per the EPUB spec, even if the library is failing to expose it well.
        1 => PathBuf::from(path_split.first().unwrap()),
        _ => {
            let _fragment = path_split.pop().unwrap();
            PathBuf::from(path_split.join("#"))
        }
    };
    TocItem {
        iri: nav_point.content,
        path,
        label: nav_point.label,
        children: nav_point.children.into_iter().map(|child| localize_toc_item_format(child, nesting_level + 1)).collect(),
        nesting_level,
    }
}

fn flatten_toc_items<'a>(toc_items: &'a Vec<TocItem>) -> Vec<&'a TocItem> {
    let mut flattened_toc_items = Vec::new();
    for toc_item in toc_items {
        flattened_toc_items.push(toc_item);
        flattened_toc_items.append(&mut flatten_toc_items(&toc_item.children));
    }
    flattened_toc_items
}

fn toc_is_linear_relative_to_spine(toc: &Vec<TocItem>, spine: &Vec<SpineItem>) -> bool {
    let mut last_spine_index = 0;
    for toc_item in flatten_toc_items(toc) {
        let toc_item_spine_index = spine
            .iter()
            .position(|spine_item| &spine_item.path == &toc_item.path)
            .expect(&format!("TOC contains path {}, which doesn't appear in book spine.", &toc_item.path.display()));
        if toc_item_spine_index < last_spine_index {
            return false
        } else {
            last_spine_index = toc_item_spine_index;
        }
    }
    true
}

fn map_toc_items_to_spine_items(toc: &Vec<TocItem>, spine: &Vec<SpineItem>) -> Vec<(SpineItem, Vec<TocItem>)> {
    let flattened_toc = flatten_toc_items(toc);
    spine.iter().map(|spine_item| (spine_item.clone(), flattened_toc.iter().filter(|toc_item| &toc_item.path == &spine_item.path).copied().cloned().collect())).collect()
}

fn list_toc_items_for_linear_index_spine_entry(toc_items: &Vec<TocItem>, book_contents_dir: &PathBuf) -> maud::Markup {
    fn list_toc_items_for_linear_index_spine_entry_recursive(book_contents_dir: &PathBuf, nesting_level: usize, toc_items_iter: &mut std::iter::Peekable<std::slice::Iter<TocItem>>) -> maud::Markup {
        html! {
            @while let Some(next_item) = toc_items_iter.peek() {
                @match nesting_level.cmp(&next_item.nesting_level) {
                    Ordering::Equal => li {
                        @let toc_item = toc_items_iter.next().unwrap();
                        a href=(book_contents_dir.join(&toc_item.iri).display()) { (toc_item.label) }
                    },
                    Ordering::Less => ul {
                        (list_toc_items_for_linear_index_spine_entry_recursive(book_contents_dir, nesting_level + 1, toc_items_iter))
                    },
                    Ordering::Greater => {
                        // @break // Inconveniently this is not yet implemented; need to fix this before release, because otherwise it'll loop infinitely
                    },
                }
            }
        }
    }

    list_toc_items_for_linear_index_spine_entry_recursive(book_contents_dir, 0, &mut toc_items.iter().peekable())
}

fn list_toc_items_for_nonlinear_index(toc_items: &Vec<TocItem>, book_contents_dir: &PathBuf) -> maud::Markup {
    html! {
        @for toc_item in toc_items {
            li {
                a href=(book_contents_dir.join(&toc_item.iri).display()) { (toc_item.label) }
            }
            @if !toc_item.children.is_empty() {
                ul {
                    (list_toc_items_for_nonlinear_index(&toc_item.children, book_contents_dir))
                }
            }
        }
    }
}

fn create_index(book: &EpubDoc<BufReader<File>>, toc: &Vec<TocItem>, spine: &Vec<SpineItem>, book_contents_dir: &PathBuf, has_stylesheet: bool) -> String {
    let title = book.mdata("title").expect("Ill-formed EPUB: doesn't have defined title metadata.");

    html!{
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                title {
                    "rib | " (title) " | Index" // Rename
                }
                @if has_stylesheet {
                    link rel="stylesheet" href="index_stylesheet.css";
                }
            }
            body style="text-align: center;" {
                h1 { (title) }
                @if let Some(creator) = book.mdata("creator") {
                    // Add support for multi-creator books?
                    h3 { (creator) }
                }
                @if let Some(cover_id) = book.get_cover_id() {
                    img alt="book cover image" src=(book_contents_dir.join(book.resources.get(&cover_id).unwrap().0.clone()).display());
                }
                p {
                    a href=(book_contents_dir.join(&spine.first().unwrap().path).display()) { "Start" }
                }
                // Bodymatter, if there's a good way to get it within the limits of this epub crate
                p {
                    a href=(book_contents_dir.join(&spine.last().unwrap().path).display()) { "End" }
                }
                table style="border-collapse: collapse; margin-left: auto; margin-right: auto;" {
                    // Factor styles out to the stylesheet probably (using the same techniques, in case of override, as are used for main book body)
                    // Make margins more consistent for list-items
                    // Have spine display show something about linearity?
                    @if toc_is_linear_relative_to_spine(&toc, &spine) {
                        tr {
                            td style="border: 1px solid black; vertical-align: top;" { "Spine" }
                            td style="border: 1px solid black; vertical-align: top;" { "Table of Contents" }
                        }
                        @for (spine_item, toc_items) in map_toc_items_to_spine_items(toc, spine) {
                            tr {
                                td style="border: 1px solid black; vertical-align: top;" {
                                    ul style="text-align: left;" {
                                        li {
                                            a href=(book_contents_dir.join(&spine_item.path).display()) { (&spine_item.path.display()) }
                                        }
                                    }
                                }
                                td style="border: 1px solid black; vertical-align: top;" {
                                    @if !toc_items.is_empty() {
                                        ul style="text-align: left;" {
                                            (list_toc_items_for_linear_index_spine_entry(&toc_items, book_contents_dir))
                                        }
                                    } @else {
                                        br;
                                    }
                                }
                            }
                        }
                    } @else {
                        tr {
                            td style="border: 1px solid black; vertical-align: top;" { "Spine" }
                            td { br; }
                            td style="border: 1px solid black; vertical-align: top;" { "Table of Contents" }
                        }
                        tr {
                            td style="border: 1px solid black; vertical-align: top;" {
                                ul style="text-align: left;" {
                                    @for spine_item in spine {
                                        @if spine_item.linear {
                                            li {
                                                a href=(book_contents_dir.join(&spine_item.path).display()) { (&spine_item.path.display()) }
                                            }
                                        }
                                    }
                                }
                            }
                            td { br; }
                            td style="border: 1px solid black; vertical-align: top;" {
                                ul style="text-align: left;" {
                                    (list_toc_items_for_nonlinear_index(&toc, book_contents_dir))
                                }
                            }
                        }
                    }
                }
            }
        }
    }.into_string()
}

fn dump_book(book: &mut EpubDoc<BufReader<File>>, index_dir: &PathBuf, stylesheet: &Stylesheet) -> usize {
    let contents_dir = index_dir.join("epub");
    let styles_dir = index_dir.join("styles");
    create_dir_all(index_dir).expect(&format!("Couldn't create cache dir {}.", index_dir.display()));
    create_dir_all(&contents_dir).expect(&format!("Couldn't create epub subdir for cache dir {}. (This shouldn't happen.)", index_dir.display()));
    create_dir_all(&styles_dir).expect(&format!("Couldn't create styles subdir for cache dir {}. (This shouldn't happen.)", styles_dir.display()));
    let index_path = index_dir.join("index.html");

    let mut dumped_bytes = 0;

    let toc = book.toc.iter().map(|nav_point| localize_toc_item_format(nav_point.clone(), 0)).collect::<Vec<TocItem>>();
    let spine = book.spine.iter().map(|spine_item_id| SpineItem {
        // Complexify once the epub crate adds support for nonlinearity
        path: book.resources.get(spine_item_id).unwrap().0.clone(),
        linear: true,
    }).collect::<Vec<SpineItem>>();
    let book_ids_and_paths = book.resources.iter().map(|(id, (path, _mimetype))| {
        (id.clone(), path.clone())
    }).collect::<Vec<(String, PathBuf)>>();
    for (id, mut path) in book_ids_and_paths {
        // This has a security hole against ill-formed EPUBs with paths leaking out of the zip container. Add some precautions there maybe.
        let (mut resource, resource_type) = book.get_resource(&id).unwrap();
        let resource_dir = contents_dir.join(path.parent().unwrap());
        create_dir_all(resource_dir).expect("Couldn't create cache subdir {}. (This shouldn't happen.)");
        if book.spine.contains(&id) {
            match resource_type.as_ref() {
                "application/xhtml+xml" => {
                    let css_path = {
                        let mut possible_path = styles_dir.join(path.file_name().unwrap());
                        possible_path.set_extension("css");
                        while possible_path.exists() {
                            let mut filename = possible_path.file_stem().unwrap().to_os_string();
                            filename.push("_.css");
                            possible_path.set_file_name(filename);
                            // This can in theory produce arbitrarily-long paths, exceeding file-system limits, if the spine contains many files differing only in extension. If a convenient solution to this issue exists, consider implementing it.
                        }
                        possible_path
                    };
                    let resource_spine_position = spine.iter().position(|spine_item| spine_item.path == path).expect("Internal spine representation is ill-formed. (If this happens, please report it.)");
                    let resource_associated_css;
                    (resource, resource_associated_css) = process_spine_xhtml(&resource, &contents_dir, &index_path, &spine, resource_spine_position, stylesheet, &css_path);
                    if let Some(css) = resource_associated_css {
                        dumped_bytes += css.len();
                        write(contents_dir.join(&css_path), css).expect(&format!("Failed to write {} from book to disk.", css_path.display()));
                    }
                },
                "image/svg+xml" => println!("Warning: books with SVG spine items currently lack navigation and stylesheet support."),
                _ => panic!("Spine contains item of type other than application/xhtml+xml or image/svg+xml.")
            }
        }
        dumped_bytes += resource.len();
        write(contents_dir.join(&path), resource).expect(&format!("Failed to write {} from book to disk.", path.display()));
    }

    let index_css = create_index_css(stylesheet);
    if let Some(css) = &index_css {
        write(index_dir.join("index_stylesheet.css"), css).expect("Failed to write index stylesheet.");
    }

    let index = create_index(book, &toc, &spine, &contents_dir, index_css.is_some());
    write(&index_path, index).expect("Failed to write index.");

    dumped_bytes
}

//////////////
//   Main   //
//////////////

fn main() {
    let args: Args = argh::from_env();

    let project_dirs = ProjectDirs::from("", "", "rib").unwrap();

    let config = Config::open(&PathBuf::from(project_dirs.config_dir()).join("config.toml"));

    let cache_path = PathBuf::from(project_dirs.cache_dir()).join("cache_index.json");
    let mut cache = Cache::open(cache_path.clone(), &config);

    let stylesheet = match args.stylesheet {
        None => config.stylesheets.get(&config.default_stylesheet).expect(&format!("Default stylesheet '{}' wasn't found in config.", config.default_stylesheet)),
        Some(sheet_name) => config.stylesheets.get(&sheet_name).expect(&format!("Stylesheet '{}' wasn't found in config.", sheet_name)),
    }.clone();

    let mut book = EpubDoc::new(args.epub.clone()).expect(&format!("Failed to open {} as epub.", args.epub));
    let book_cache_id = match book.get_release_identifier() {
        Some(release_id) => release_id,
        None => book.unique_identifier.clone().expect("Ill-formed EPUB: doesn't have unique identifier."),
    };
    let book_cache_dirname = {
        let sanitized_id = sanitize_filename::sanitize(&book_cache_id);
        if cache.contents.iter().any(|cached_book| cached_book.path == PathBuf::from(&sanitized_id)) {
            let mut numeric_extension = 2;
            let mut sanitized_id_plus_numeric_extension = format!("{}_{}", sanitized_id, numeric_extension);
            while cache.contents.iter().any(|cached_book| cached_book.path == PathBuf::from(&sanitized_id_plus_numeric_extension)) {
                numeric_extension += 1;
                sanitized_id_plus_numeric_extension = format!("{}_{}", sanitized_id, numeric_extension)
            }
            sanitized_id_plus_numeric_extension
        } else {
            sanitized_id
        }
    };
    let book_cache_dir_path = cache_path.parent().unwrap().join(&book_cache_dirname);

    let dumped_bytes = dump_book(&mut book, &book_cache_dir_path, &stylesheet);
    cache.add(book_cache_id, book_cache_dirname, dumped_bytes);

    if !args.browser_skip {
        let browser = match args.browser {
            Some(browser) => browser,
            None => config.default_browser,
        };
        Command::new(browser)
            .arg(book_cache_dir_path.join("index.html"))
            .output()
            .expect("Failed to open in browser.");
    }
}
