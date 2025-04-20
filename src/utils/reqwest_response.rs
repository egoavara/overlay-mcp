use axum::response::IntoResponse;
pub struct ReqwestResponse(pub reqwest::Response);

impl From<reqwest::Response> for ReqwestResponse {
    fn from(response: reqwest::Response) -> Self {
        Self(response)
    }
}

impl IntoResponse for ReqwestResponse {
    fn into_response(self) -> axum::response::Response {
        let mut builder = axum::response::Response::builder().status(self.0.status());
        for (key, value) in self.0.headers() {
            builder = builder.header(key, value);
        }
        let body = axum::body::Body::from_stream(self.0.bytes_stream());
        builder.body(body).unwrap()
    }
}
