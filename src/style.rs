use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use serde::{Deserialize, Deserializer, Serialize};

use crate::cli::CliStyleCommands;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct StylesheetValue {
    pub value: String,
    pub override_book: bool,
}

impl StylesheetValue {
    fn new(value: String, override_book: bool) -> Self {
        Self {
            value,
            override_book,
        }
    }

    fn with_overrides(
        &self,
        value_override: &Option<String>,
        override_book_override: Option<bool>,
    ) -> Self {
        Self {
            value: value_override
                .as_ref()
                .cloned()
                .unwrap_or(self.value.clone()),
            override_book: override_book_override.unwrap_or(self.override_book),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RawStylesheet {
    text_color: Option<String>,
    link_color: Option<String>,
    background_color: Option<String>,
    margin_size: Option<String>,
    max_image_width: Option<String>,

    #[serde(default)]
    text_color_override: bool,
    #[serde(default)]
    link_color_override: bool,
    #[serde(default)]
    background_color_override: bool,
    #[serde(default)]
    margin_size_override: bool,
    #[serde(default)]
    max_image_width_override: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Stylesheet {
    pub text_color: Option<StylesheetValue>,
    pub link_color: Option<StylesheetValue>,
    pub background_color: Option<StylesheetValue>,
    pub margin_size: Option<StylesheetValue>,
    pub max_image_width: Option<StylesheetValue>,
}

impl From<RawStylesheet> for Stylesheet {
    fn from(value: RawStylesheet) -> Self {
        Self {
            text_color: value
                .text_color
                .map(|color| StylesheetValue::new(color, value.text_color_override)),
            link_color: value
                .link_color
                .map(|color| StylesheetValue::new(color, value.link_color_override)),
            background_color: value
                .background_color
                .map(|color| StylesheetValue::new(color, value.background_color_override)),
            margin_size: value
                .margin_size
                .map(|margin| StylesheetValue::new(margin, value.margin_size_override)),
            max_image_width: value
                .max_image_width
                .map(|width| StylesheetValue::new(width, value.max_image_width_override)),
        }
    }
}

impl Stylesheet {
    pub fn deserialize_config_map<'a, 'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashMap<String, Self>, D::Error> {
        let raw_config_map = HashMap::<String, RawStylesheet>::deserialize(deserializer)?;
        Ok(raw_config_map
            .into_iter()
            .map(|(name, raw_stylesheet)| (name, raw_stylesheet.into()))
            .collect())
    }

    pub fn is_null(&self) -> bool {
        self.text_color.is_none()
            && self.link_color.is_none()
            && self.background_color.is_none()
            && self.margin_size.is_none()
            && self.max_image_width.is_none()
    }

    pub fn with_overrides(&self, overrides: &CliStyleCommands) -> Self {
        Self {
            text_color: self
                .text_color
                .as_ref()
                .map(|value| {
                    value.with_overrides(&overrides.text_color, overrides.text_color_override)
                })
                .or_else(|| {
                    overrides.text_color.as_ref().cloned().map(|color| {
                        StylesheetValue::new(
                            color.clone(),
                            overrides.text_color_override.unwrap_or_default(),
                        )
                    })
                }),
            link_color: self
                .link_color
                .as_ref()
                .map(|value| {
                    value.with_overrides(&overrides.link_color, overrides.link_color_override)
                })
                .or_else(|| {
                    overrides.link_color.as_ref().cloned().map(|color| {
                        StylesheetValue::new(
                            color,
                            overrides.link_color_override.unwrap_or_default(),
                        )
                    })
                }),
            background_color: self
                .background_color
                .as_ref()
                .map(|value| {
                    value.with_overrides(
                        &overrides.background_color,
                        overrides.background_color_override,
                    )
                })
                .or_else(|| {
                    overrides.background_color.as_ref().cloned().map(|color| {
                        StylesheetValue::new(
                            color,
                            overrides.background_color_override.unwrap_or_default(),
                        )
                    })
                }),
            margin_size: self
                .margin_size
                .as_ref()
                .map(|value| {
                    value.with_overrides(&overrides.margin_size, overrides.margin_size_override)
                })
                .or_else(|| {
                    overrides.margin_size.as_ref().cloned().map(|margin| {
                        StylesheetValue::new(
                            margin,
                            overrides.margin_size_override.unwrap_or_default(),
                        )
                    })
                }),
            max_image_width: self
                .max_image_width
                .as_ref()
                .map(|value| {
                    value.with_overrides(
                        &overrides.max_image_width,
                        overrides.max_image_width_override,
                    )
                })
                .or_else(|| {
                    overrides.max_image_width.as_ref().cloned().map(|width| {
                        StylesheetValue::new(
                            width,
                            overrides.max_image_width_override.unwrap_or_default(),
                        )
                    })
                }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Style {
    pub include_index: bool,
    pub inject_navigation: bool,
    pub stylesheet: Option<Stylesheet>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            include_index: true,
            inject_navigation: true,
            stylesheet: None,
        }
    }
}

impl Style {
    pub const fn raw() -> Self {
        Self {
            include_index: false,
            inject_navigation: false,
            stylesheet: None,
        }
    }

    pub fn get_default_hash(&self) -> u64 {
        let mut default_hasher = DefaultHasher::new();
        self.hash(&mut default_hasher);
        default_hasher.finish()
    }

    pub const fn uses_raw_contents_dir(&self) -> bool {
        !(self.inject_navigation || self.stylesheet.is_some())
    }

    pub fn text_color(&self) -> Option<&StylesheetValue> {
        self.stylesheet
            .as_ref()
            .and_then(|stylesheet| stylesheet.text_color.as_ref())
    }

    pub fn link_color(&self) -> Option<&StylesheetValue> {
        self.stylesheet
            .as_ref()
            .and_then(|stylesheet| stylesheet.link_color.as_ref())
    }

    pub fn background_color(&self) -> Option<&StylesheetValue> {
        self.stylesheet
            .as_ref()
            .and_then(|stylesheet| stylesheet.background_color.as_ref())
    }

    pub fn margin_size(&self) -> Option<&StylesheetValue> {
        self.stylesheet
            .as_ref()
            .and_then(|stylesheet| stylesheet.margin_size.as_ref())
    }

    pub fn max_image_width(&self) -> Option<&StylesheetValue> {
        self.stylesheet
            .as_ref()
            .and_then(|stylesheet| stylesheet.max_image_width.as_ref())
    }
}
