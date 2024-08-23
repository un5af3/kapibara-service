//! Service Option

use serde::{Deserialize, Serialize};

use crate::{
    socks::{SocksInboundOption, SocksOutboundOption},
    vless::{VlessInboundOption, VlessOutboundOption},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboundServiceOption {
    Vless(VlessInboundOption),
    Socks(SocksInboundOption),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundServiceOption {
    Direct,
    Vless(VlessOutboundOption),
    Socks(SocksOutboundOption),
}
