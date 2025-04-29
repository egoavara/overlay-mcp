use core::fmt;
use std::{borrow::Cow, sync::LazyLock};

use anyhow::Result;
use http::request::Parts;
use json_patch::jsonptr::PointerBuf;
use regex::Regex;
use serde::{de::Visitor, Deserialize, Serialize};

use super::JsonValue;

#[derive(Debug, Clone)]
pub enum HttpPartReference {
    Header(String),
    Query(String),
    HeaderRegex(Regex),
    QueryRegex(Regex),
    Context(PointerBuf),
    Unsafe(PointerBuf),
}

impl HttpPartReference {
    pub(crate) fn resolve_http_part<'a>(
        &'a self,
        parts: &'a Parts,
    ) -> Box<dyn Iterator<Item = Cow<'a, str>> + 'a> {
        match self {
            Self::Header(s) => {
                let temp = parts
                    .headers
                    .get(s)
                    .into_iter()
                    .filter_map(|v| v.to_str().ok().map(Cow::Borrowed));
                Box::new(temp)
            }
            Self::Query(s) => {
                let Some(val) = parts.uri.query() else {
                    return Box::new(std::iter::empty());
                };
                // let temp = form_urlencoded::parse(val.as_bytes())
                //     .into_iter()
                // Box::new(temp)
                let val_bytes = val.as_bytes();
                let found = form_urlencoded::parse(val_bytes)
                    .into_iter()
                    .filter(|(k, _)| k.as_ref() == s.as_str())
                    .map(|(_, v)| v);
                Box::new(found)
            }
            Self::HeaderRegex(r) => Box::new(
                parts
                    .headers
                    .iter()
                    .filter(|(k, _)| r.is_match(k.as_ref()))
                    .filter_map(|(k, v)| {
                        if !r.is_match(k.as_str()) {
                            return None;
                        }
                        v.to_str().ok().map(Cow::Borrowed)
                    }),
            ),
            Self::QueryRegex(r) => {
                let Some(val) = parts.uri.query() else {
                    return Box::new(std::iter::empty());
                };
                let temp = form_urlencoded::parse(val.as_ref())
                    .into_iter()
                    .filter(|(k, _)| r.is_match(k.as_ref()))
                    .map(|(_, v)| v);
                Box::new(temp)
            }
            Self::Context(_) => Box::new(std::iter::empty()),
            Self::Unsafe(_) => Box::new(std::iter::empty()),
        }
    }

    pub fn resolve<'a>(&self, parts: &'a JsonValue) -> Vec<&'a JsonValue> {
        match self {
            Self::Header(s) => {
                let temp = parts
                    .get("header")
                    .and_then(|v| v.get(s))
                    .into_iter()
                    .collect();
                temp
            }
            Self::Query(s) => {
                let temp = parts
                    .get("query")
                    .and_then(|v| v.get(s))
                    .into_iter()
                    .collect();
                temp
            }
            Self::HeaderRegex(r) => {
                let headers = parts.get("header").unwrap();
                headers
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, _)| r.is_match(k))
                    .map(|(_, v)| v)
                    .collect()
            }
            Self::QueryRegex(r) => {
                let query = parts.get("query").unwrap();
                query
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, _)| r.is_match(k))
                    .map(|(_, v)| v)
                    .collect()
            }
            Self::Context(p) => {
                let ctx = parts.get("context").unwrap();
                let Ok(found) = p.resolve(ctx) else {
                    return Vec::new();
                };
                vec![found]
            }
            Self::Unsafe(p) => {
                let Ok(found) = p.resolve(parts) else {
                    return Vec::new();
                };
                vec![found]
            }
        }
    }
}

static REGEX_VALUE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/[.+]/$").unwrap());

impl Serialize for HttpPartReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Header(s) => serializer.serialize_str(&format!("header:{}", s)),
            Self::Query(s) => serializer.serialize_str(&format!("query:{}", s)),
            Self::HeaderRegex(r) => serializer.serialize_str(&format!("header:/{}/", r)),
            Self::QueryRegex(r) => serializer.serialize_str(&format!("query:/{}/", r)),
            Self::Context(p) => serializer.serialize_str(&format!("context:{}", p)),
            Self::Unsafe(p) => serializer.serialize_str(&format!("unsafe:{}", p)),
        }
    }
}

impl<'de> Deserialize<'de> for HttpPartReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ReferenceVisitor;

        impl Visitor<'_> for ReferenceVisitor {
            type Value = HttpPartReference;

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
                    ("header", false) => Ok(HttpPartReference::Header(value.to_string())),
                    ("query", false) => Ok(HttpPartReference::Query(value.to_string())),
                    ("header", true) => Regex::new(value)
                        .map(HttpPartReference::HeaderRegex)
                        .map_err(E::custom),
                    ("query", true) => Regex::new(value)
                        .map(HttpPartReference::QueryRegex)
                        .map_err(E::custom),
                    ("context", _) => Ok(HttpPartReference::Context(
                        PointerBuf::parse(value.to_string()).map_err(E::custom)?,
                    )),
                    ("unsafe", _) => Ok(HttpPartReference::Unsafe(
                        PointerBuf::parse(value.to_string()).map_err(E::custom)?,
                    )),
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
