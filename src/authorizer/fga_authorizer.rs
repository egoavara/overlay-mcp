use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Map, DisplayFromStr};
use url::Url;

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FgaAuthorizer {
    pub uri: Url,

    #[serde_as(as = "Map<DisplayFromStr, _>")]
    pub headers: Vec<(HeaderName, String)>,
}