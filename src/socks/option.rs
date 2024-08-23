//! Socks service option

use serde::{Deserialize, Serialize};

use super::protocol::SocksAuth;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksInboundOption {
    #[serde(default)]
    pub auth: Vec<SocksAuthOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksOutboundOption {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub auth: SocksAuthOption,
}

fn default_version() -> u8 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocksAuthOption {
    NoAuth,
    Socks4(String),
    Username { user: String, pass: String },
}

impl Default for SocksAuthOption {
    fn default() -> Self {
        Self::NoAuth
    }
}

impl From<SocksAuthOption> for SocksAuth {
    fn from(value: SocksAuthOption) -> Self {
        match value {
            SocksAuthOption::NoAuth => SocksAuth::NoAuth,
            SocksAuthOption::Socks4(data) => SocksAuth::Socks4(data.into_bytes()),
            SocksAuthOption::Username { user, pass } => {
                SocksAuth::Username(user.into_bytes(), pass.into_bytes())
            }
        }
    }
}
