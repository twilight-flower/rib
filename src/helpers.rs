use std::{
    fs::{read_dir, symlink_metadata},
    path::PathBuf,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serializer, de::Error};

////////////
//   fs   //
////////////

pub fn get_dir_size(path: &PathBuf) -> u64 {
    // Doesn't follow symlinks. Could be pretty easily modded to do so if useful later.
    read_dir(path)
        .expect(&format!("Couldn't read {} as directory.", path.display()))
        .fold(0, |bytes, maybe_dir_entry| {
            let dir_entry =
                maybe_dir_entry.expect(&format!("Error viewing entry in {}", path.display()));
            let dir_entry_path = dir_entry.path();
            let dir_entry_metadata = symlink_metadata(&dir_entry_path).expect(&format!(
                "Couldn't read metadata for {}.",
                dir_entry_path.display()
            ));
            bytes
                + match dir_entry_metadata.is_dir() {
                    true => get_dir_size(&dir_entry_path),
                    false => dir_entry_metadata.len(),
                }
        })
}

//////////////
//   path   //
//////////////

pub fn make_pathbuf_separators_consistent(pathbuf_in: &PathBuf) -> PathBuf {
    pathbuf_in
        .components()
        .fold(PathBuf::new(), |mut pathbuf_out, component| {
            pathbuf_out.push(component);
            pathbuf_out
        })
}

///////////////
//   serde   //
///////////////

pub fn deserialize_datetime<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<DateTime<Utc>, D::Error> {
    let datetime_string = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&datetime_string)
        .map(|datetime| datetime.to_utc())
        .map_err(|_| D::Error::custom("Couldn't parse deserialized string as RFC 3339 datetime."))
}

pub fn serialize_datetime<S: Serializer>(
    datetime: &DateTime<Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let datetime_string = datetime.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    serializer.serialize_str(&datetime_string)
}
