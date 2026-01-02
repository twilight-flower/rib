use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Style {
    pub include_index: bool,
    // pub inject_navigation: bool,
    // Stylesheet details go here once defined
}

impl Default for Style {
    fn default() -> Self {
        Self {
            include_index: true,
        }
    }
}

impl Style {
    pub fn raw() -> Self {
        Self {
            include_index: false,
        }
    }

    pub fn get_default_hash(&self) -> u64 {
        let mut default_hasher = DefaultHasher::new();
        self.hash(&mut default_hasher);
        default_hasher.finish()
    }
}
