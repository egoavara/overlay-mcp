use http::request;

pub trait MCPSpecification {
    fn extract_session_id(header: &request::Parts) -> Option<String>;
}
pub struct MCP20241105;
pub struct MCP20250326;

impl MCPSpecification for MCP20241105 {
    fn extract_session_id(header: &request::Parts) -> Option<String> {
        let queryparam = header.uri.query()?;
        
        form_urlencoded::parse(queryparam.as_bytes())
            .find(|(key, _)| key == "session_id")
            .map(|(_, value)| value.to_string())
    }
}
impl MCPSpecification for MCP20250326 {
    fn extract_session_id(header: &request::Parts) -> Option<String> {
        let session_id = header.headers.get("MCP-Session-Id");
        session_id.map(|value| value.to_str().unwrap().to_string())
    }
}
