use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serializer, de::Error};

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
