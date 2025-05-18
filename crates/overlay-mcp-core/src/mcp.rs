use std::borrow::Cow;

use http::request;
use url::form_urlencoded;

pub trait MCP {
    fn version() -> &'static str;
    fn pick_session_id(parts: &request::Parts) -> Option<Cow<'_, str>>;
}

pub struct MCP20241105;

impl MCP for MCP20241105 {
    fn version() -> &'static str {
        "20241105"
    }

    fn pick_session_id(parts: &request::Parts) -> Option<Cow<'_, str>> {
        let querystring = parts.uri.query().unwrap_or("");

        form_urlencoded::parse(querystring.as_bytes())
            .find(|(key, _)| key == "session_id")
            .map(|(_, value)| value)
    }
}
