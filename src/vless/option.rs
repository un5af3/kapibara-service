use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessInboundOption {
    pub users: Vec<VlessUserOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessUserOption {
    pub user: String,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessOutboundOption {
    pub uuid: String,
    pub flow: Option<String>,
}
