use regex::Regex;

use super::{reference::HttpPartReference, BaseModifiers, Modifer, Passthrough};

impl Default for BaseModifiers {
    fn default() -> Self {
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers
        Self(vec![
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::Header("date".to_string()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::Header("cookie".to_string()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::Header("user-agent".to_string()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::Header("x-real-ip".to_string()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::HeaderRegex(Regex::new(r"^x-forwarded").unwrap()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::HeaderRegex(Regex::new(r"^forwarded(-.+)+").unwrap()),
            }),
            Modifer::Passthrough(Passthrough::Implicit {
                from: HttpPartReference::HeaderRegex(Regex::new(r"^sec-ch-ua").unwrap()),
            }),
        ])
    }
}
