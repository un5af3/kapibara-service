//! Http Proxy Option

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpInboundOption {
    #[serde(default)]
    pub auth: Vec<HttpAuthOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAuthOption {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpOutboundOption {
    #[serde(default)]
    pub auth: Option<HttpAuthOption>,
}
