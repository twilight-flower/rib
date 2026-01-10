use std::{collections::HashMap, hash::{DefaultHasher, Hash, Hasher}};

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct StylesheetValue {
    value: String,
    override_book: bool,
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

impl Into<Stylesheet> for RawStylesheet {
    fn into(self) -> Stylesheet {
        Stylesheet {
            text_color: self.text_color.map(|color| StylesheetValue {
                value: color,
                override_book: self.text_color_override
            }),
            link_color: self.link_color.map(|color| StylesheetValue {
                value: color,
                override_book: self.link_color_override
            }),
            background_color: self.background_color.map(|color| StylesheetValue {
                value: color,
                override_book: self.background_color_override
            }),
            margin_size: self.margin_size.map(|color| StylesheetValue {
                value: color,
                override_book: self.margin_size_override
            }),
            max_image_width: self.max_image_width.map(|color| StylesheetValue {
                value: color,
                override_book: self.max_image_width_override
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Stylesheet {
    pub text_color: Option<StylesheetValue>,
    pub link_color: Option<StylesheetValue>,
    pub background_color: Option<StylesheetValue>,
    pub margin_size: Option<StylesheetValue>,
    pub max_image_width: Option<StylesheetValue>,
}

impl Stylesheet {
    pub fn deserialize_config_map<'a, 'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashMap<String, Self>, D::Error> {
        let raw_config_map = HashMap::<String, RawStylesheet>::deserialize(deserializer)?;
        Ok(raw_config_map.into_iter().map(|(name, raw_stylesheet)| (name, raw_stylesheet.into())).collect())
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
    pub fn raw() -> Self {
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

    pub fn uses_raw_contents_dir(&self) -> bool {
        !self.inject_navigation || self.stylesheet.is_some()
    }
}
