mod default_modifier;
mod modifer_apply;
pub mod reference;
pub mod restore;
use std::collections::HashMap;

use anyhow::{Context, Result};
use http::{HeaderMap, Method, Uri};
use json_patch::jsonptr::PointerBuf;
use reference::HttpPartReference;
use restore::{RestoreDestination, RestoreSource, RestoreUrl};
use serde::{Deserialize, Serialize};
pub type JsonValue = serde_json::Value;

pub struct ReqModifier {
    /// The source of the request
    /// ```json
    /// {
    ///     "header": {
    ///         "sec-ch-ua": "Chrome",
    ///         "sec-ch-ua-arch": "x86",
    ///         "sec-ch-ua-bitness": "64",
    ///         "sec-ch-ua-form-factors": "desktop",
    ///         "sec-ch-ua-mobile": false,
    ///         "sec-ch-ua-model": "Macintosh",
    ///         "sec-ch-ua-platform": "macOS",
    ///         "sec-ch-ua-platform-version": "13.4",
    ///     },
    ///     "query": {
    ///         "q": ["hello"],
    ///     },
    ///     "url": {
    ///         "scheme": "http",        // must exists
    ///         "host": "example.com",   // must exists
    ///         "port": 8080,            // optional
    ///         "path": "/test",
    ///     },
    ///     "context": {
    ///         "upstream": "http://10.1.1.10:8080/sse", // must exists
    ///         "session":{
    ///             "id": "1234567890",                  // must exists
    ///         }
    ///     }
    /// }
    /// ```
    pub src: JsonValue,
    /// The destination of the request
    /// Same as src, but no overlay config
    pub dst: JsonValue,
}

fn parse_header(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect()
}

fn parse_query(query: Option<&str>) -> HashMap<String, Vec<String>> {
    let Some(query) = query else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for (k, v) in form_urlencoded::parse(query.as_bytes()) {
        map.entry(k.to_string())
            .or_insert_with(Vec::new)
            .push(v.to_string());
    }
    map
}

fn parse_url(url: &Uri) -> RestoreUrl {
    tracing::info!("url: {}", url.to_string());
    let authority = url.authority();
    let host = authority.map(|x| x.host().to_string());
    let port = authority.and_then(|x| x.port_u16());

    RestoreUrl {
        scheme: url.scheme().map(|x| x.to_string()),
        host,
        port,
        path: url.path().to_string(),
    }
}

impl ReqModifier {
    pub fn new(src: &http::request::Parts, context: JsonValue) -> Self {
        let src = RestoreSource {
            method: src.method.to_string(),
            header: parse_header(&src.headers),
            query: parse_query(src.uri.query()),
            url: parse_url(&src.uri),
            context,
        };
        let dst = RestoreDestination {
            method: Method::GET.to_string(),
            header: HashMap::new(),
            query: HashMap::new(),
            url: src.url.clone(),
        };
        Self {
            src: serde_json::to_value(src).unwrap(),
            dst: serde_json::to_value(dst).unwrap(),
        }
    }
    pub fn new_dst(
        src: http::request::Parts,
        context: JsonValue,
        dst_method: Method,
        dst_uri: Uri,
        dst_headers: HeaderMap,
    ) -> Self {
        let src = RestoreSource {
            method: dst_method.to_string(),
            header: parse_header(&src.headers),
            query: parse_query(src.uri.query()),
            url: parse_url(&src.uri),
            context,
        };
        let dst = RestoreDestination {
            method: dst_method.to_string(),
            header: parse_header(&dst_headers),
            query: parse_query(dst_uri.query()),
            url: parse_url(&dst_uri),
        };
        Self {
            src: serde_json::to_value(src).unwrap(),
            dst: serde_json::to_value(dst).unwrap(),
        }
    }

    pub fn extract_first(&self, req: &HttpPartReference) -> Option<JsonValue> {
        let parts = req.resolve(&self.src);
        parts.first().map(|&x| x.clone())
    }

    pub fn extract_first_str(&self, req: &HttpPartReference) -> Option<String> {
        let parts = req.resolve(&self.src);
        for part in parts {
            if let Some(array) = part.as_array() {
                if let Some(s) = array.iter().filter_map(|x| x.as_str()).next() {
                    return Some(s.to_string());
                }
            }
            if let Some(s) = part.as_str() {
                return Some(s.to_string());
            }
        }
        None
    }

    pub fn apply(&mut self, req: &BaseModifiers) -> Result<()> {
        let src = &self.src;
        let dst = &mut self.dst;
        for modifier in &req.0 {
            modifier.apply(src, dst)?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<RestoreDestination> {
        let dest = serde_json::from_value::<RestoreDestination>(self.dst)
            .context("failed to deserialize dst, please check the config")?;
        Ok(dest)
    }
    pub fn finish_client(self) -> Result<reqwest::Client> {
        let dst = self.finish()?;
        let mut client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::limited(3));
        client = client.default_headers(dst.build_header_map());
        client.build().context("failed to build client")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseModifiers(Vec<Modifer>);

impl BaseModifiers {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Modifer {
    Passthrough(Passthrough),
    Replace(Replace),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Passthrough {
    Explicit {
        from: HttpPartReference,
        to: HttpPartReference,
    },
    Implicit {
        from: HttpPartReference,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replace {
    pub to: HttpPartReference,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromContext {
    pub to: HttpPartReference,
    #[serde(with = "json_ptr_serde")]
    pub context: PointerBuf,
}

mod json_ptr_serde {
    use json_patch::jsonptr::PointerBuf;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &PointerBuf, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PointerBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        PointerBuf::parse(&s).map_err(serde::de::Error::custom)
    }
}
