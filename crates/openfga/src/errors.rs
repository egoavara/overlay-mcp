use http::header::{InvalidHeaderName, InvalidHeaderValue};

use crate::OpenfgaFailure;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Url parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Serde url encode error: {0}")]
    SerdeUrlEncode(#[from] serde_urlencoded::ser::Error),
    #[error("Store not found")]
    StoreNotFound,
    #[error("Check failed: code: {}, message: {}", .0.code, .0.message)]
    CheckFailed(OpenfgaFailure),
}
