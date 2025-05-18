use http_reference::HttpReference;

pub mod builder;
pub mod errors;
pub mod http_reference;

pub fn resolve(http_ref: &HttpReference, src: &http::request::Parts) -> Option<String> {
    match http_ref {
        HttpReference::Header(s) => src
            .headers
            .get(s)
            .map(|v| v.to_str().unwrap_or_default().to_string()),
        HttpReference::Query(s) => {
            let querystring = src.uri.query().unwrap_or_default();
            form_urlencoded::parse(querystring.as_bytes())
                .find(|(k, _)| k == s)
                .map(|(_, v)| v.to_string())
        }
    }
}
