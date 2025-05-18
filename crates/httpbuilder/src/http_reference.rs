use std::{
    fmt::{self, Display},
    sync::LazyLock,
};

use regex::Regex;
use serde::{de::Visitor, Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum HttpReference {
    Header(String),
    Query(String),
}

#[derive(Debug, Clone)]
pub enum HttpMultiReference {
    Header(String),
    Query(String),
    HeaderRegex(Regex),
    QueryRegex(Regex),
}

static REGEX_VALUE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/[.+]/$").unwrap());

impl Serialize for HttpReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Serialize for HttpMultiReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for HttpReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let decoded = HttpMultiReference::deserialize(deserializer);
        match decoded {
            Ok(HttpMultiReference::Header(s)) => Ok(HttpReference::Header(s)),
            Ok(HttpMultiReference::Query(s)) => Ok(HttpReference::Query(s)),
            Ok(HttpMultiReference::HeaderRegex(_)) => Err(serde::de::Error::custom(
                "header regex cannot be deserialized",
            )),
            Ok(HttpMultiReference::QueryRegex(_)) => Err(serde::de::Error::custom(
                "query regex cannot be deserialized",
            )),
            Err(e) => Err(e),
        }
    }
}

impl<'de> Deserialize<'de> for HttpMultiReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReferenceVisitor;

        impl Visitor<'_> for ReferenceVisitor {
            type Value = HttpMultiReference;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a reference to a header or query")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let splited = v.split(":").collect::<Vec<&str>>();
                let [prefix, value, ..] = splited.as_slice() else {
                    return Err(E::custom("expected format: header:name or query:name or header:/{regex}/ or query:/{regex}/ or context:{json pointer}"));
                };
                let is_regex = REGEX_VALUE_PATTERN.is_match(value);
                match (*prefix, is_regex) {
                    ("header", false) => Ok(HttpMultiReference::Header(value.to_string())),
                    ("query", false) => Ok(HttpMultiReference::Query(value.to_string())),
                    ("header", true) => Regex::new(value)
                        .map(HttpMultiReference::HeaderRegex)
                        .map_err(E::custom),
                    ("query", true) => Regex::new(value)
                        .map(HttpMultiReference::QueryRegex)
                        .map_err(E::custom),
                    (prefix, _) => Err(E::custom(format!(
                        "expected format: header:{{...}} or query:{{...}} or context:{{...}} or unsafe:{{...}} but got {}:{{...}}",
                        prefix
                    ))),
                }
            }
        }

        deserializer.deserialize_str(ReferenceVisitor)
    }
}

impl Display for HttpReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(s) => write!(f, "header:{}", s),
            Self::Query(s) => write!(f, "query:{}", s),
        }
    }
}

impl Display for HttpMultiReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(s) => write!(f, "header:{}", s),
            Self::Query(s) => write!(f, "query:{}", s),
            Self::HeaderRegex(r) => write!(f, "header:/{}/", r.as_str()),
            Self::QueryRegex(r) => write!(f, "query:/{}/", r.as_str()),
        }
    }
}
