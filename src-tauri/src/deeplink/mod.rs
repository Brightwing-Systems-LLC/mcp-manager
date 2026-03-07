use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepLinkAction {
    pub action: String,       // "install", "uninstall", "view"
    pub server_uuid: String,
    pub tool_id: Option<String>,
}

pub struct DeepLinkState {
    pub pending: Mutex<Option<DeepLinkAction>>,
}

impl DeepLinkState {
    pub fn new() -> Self {
        DeepLinkState {
            pending: Mutex::new(None),
        }
    }
}

/// Parse a brightwing:// URL into a DeepLinkAction
pub fn parse_deep_link(url: &str) -> Option<DeepLinkAction> {
    // brightwing://install?server={uuid}&tool={tool_id}
    // brightwing://uninstall?server={uuid}
    // brightwing://view?server={uuid}
    let url = url.trim();

    let without_scheme = url.strip_prefix("brightwing://")?;
    let (action, query) = if let Some(idx) = without_scheme.find('?') {
        (&without_scheme[..idx], &without_scheme[idx + 1..])
    } else {
        (without_scheme, "")
    };

    let action = action.trim_end_matches('/');

    if !["install", "uninstall", "view"].contains(&action) {
        return None;
    }

    let params: std::collections::HashMap<&str, &str> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();

    let server_uuid = params.get("server")?.to_string();

    Some(DeepLinkAction {
        action: action.to_string(),
        server_uuid,
        tool_id: params.get("tool").map(|s| s.to_string()),
    })
}
