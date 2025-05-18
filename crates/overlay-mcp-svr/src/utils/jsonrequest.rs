use axum::{
    body::Body,
    extract::{FromRequest, Request},
    response::{IntoResponse, Response},
    Error, Json, RequestExt,
};
use http::{header, request::Parts, HeaderMap, StatusCode};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

pub struct JsonRequest<T> {
    pub parts: Parts,
    pub json: T,
}

impl<S, T> FromRequest<S> for JsonRequest<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
    Json<T>: FromRequest<S>,
{
    type Rejection = Response;

    async fn from_request(req: Request, _: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.with_limited_body().into_parts();

        if !json_content_type(&parts.headers) {
            return Err(Response::builder()
                .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
                .body(Body::from("missing json content type"))
                .unwrap());
        }

        let bytes = match body.collect().await {
            Ok(body) => body.to_bytes(),
            Err(err) => {
                let box_error = match err.into_inner().downcast::<Error>() {
                    Ok(err) => err.into_inner(),
                    Err(err) => err,
                };
                let box_error = match box_error.downcast::<Error>() {
                    Ok(err) => err.into_inner(),
                    Err(err) => err,
                };
                return Err(
                    match box_error.downcast::<http_body_util::LengthLimitError>() {
                        Ok(_) => Response::builder()
                            .status(StatusCode::PAYLOAD_TOO_LARGE)
                            .body(Body::from("Failed to buffer the request body"))
                            .unwrap(),
                        Err(_) => Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from("Failed to buffer the request body"))
                            .unwrap(),
                    },
                );
            }
        };

        let Json(value): Json<T> = Json::from_bytes(&bytes).map_err(|err| err.into_response())?;

        Ok(JsonRequest { parts, json: value })
    }
}

fn json_content_type(headers: &HeaderMap) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    let mime = if let Ok(mime) = content_type.parse::<mime::Mime>() {
        mime
    } else {
        return false;
    };

    let is_json_content_type = mime.type_() == "application"
        && (mime.subtype() == "json" || mime.suffix().is_some_and(|name| name == "json"));

    is_json_content_type
}
