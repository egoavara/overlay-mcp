use std::{collections::HashMap, str::FromStr};

use http::{HeaderName, Uri};

use crate::{
    errors::HttpBuilderError,
    http_reference::HttpMultiReference,
};

pub struct HttpBuilder<'a> {
    src: &'a http::request::Parts,
    src_query: HashMap<String, Vec<String>>,

    pub scheme: http::uri::Scheme,
    pub hostname: http::uri::Authority,
    pub path: String,
    pub query: HashMap<String, Vec<String>>,
    pub headers: http::HeaderMap,
}

pub struct Undefined;

impl<'a> HttpBuilder<'a> {
    pub fn new(
        src: &'a http::request::Parts,
        dst: Uri,
    ) -> Result<HttpBuilder<'a>, HttpBuilderError> {
        let scheme = dst.scheme().ok_or(HttpBuilderError::NoScheme)?;
        let hostname = dst.authority().ok_or(HttpBuilderError::NoHostname)?;
        let path = dst.path();
        let decoded_query = dst.query().ok_or(HttpBuilderError::NoQuery)?;
        let mut query = HashMap::new();
        let mut src_query = HashMap::new();
        for (k, v) in form_urlencoded::parse(decoded_query.as_bytes()) {
            query
                .entry(k.to_string())
                .or_insert(Vec::new())
                .push(v.to_string());
        }
        let src_decoded_query = src.uri.query().ok_or(HttpBuilderError::NoQuery)?;
        for (k, v) in form_urlencoded::parse(src_decoded_query.as_bytes()) {
            src_query
                .entry(k.to_string())
                .or_insert(Vec::new())
                .push(v.to_string());
        }
        let headers = http::HeaderMap::new();
        Ok(HttpBuilder {
            src,
            src_query,
            scheme: scheme.clone(),
            hostname: hostname.clone(),
            path: path.to_string(),
            query,
            headers,
        })
    }

    pub fn apply<'b, A: IntoIterator<Item = &'b HttpMultiReference>>(
        mut self,
        elem_iter: A,
    ) -> Result<Self, HttpBuilderError> {
        for elem in elem_iter.into_iter() {
            match elem {
                HttpMultiReference::Header(s) => {
                    if let Some(value) = self.src.headers.get(s) {
                        self.headers
                            .append(HeaderName::from_str(s).unwrap(), value.clone());
                    }
                }
                HttpMultiReference::Query(s) => {
                    if let Some(value) = self.src_query.get(s) {
                        self.query
                            .entry(s.clone())
                            .or_default()
                            .extend(value.iter().cloned());
                    }
                }
                HttpMultiReference::HeaderRegex(r) => {
                    for (k, v) in self.src.headers.iter() {
                        if r.is_match(k.as_str()) {
                            self.headers
                                .append(HeaderName::from_str(k.as_str()).unwrap(), v.clone());
                        }
                    }
                }
                HttpMultiReference::QueryRegex(r) => {
                    for (k, v) in self.src_query.iter() {
                        if r.is_match(k.as_str()) {
                            self.query
                                .entry(k.clone())
                                .or_default()
                                .extend(v.iter().cloned());
                        }
                    }
                }
            }
        }
        Ok(self)
    }
}
