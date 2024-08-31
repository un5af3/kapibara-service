//! Service Option

use serde::{Deserialize, Serialize};

use crate::{
    http::{HttpInboundOption, HttpOutboundOption},
    mixed::MixedInboundOption,
    socks::{SocksInboundOption, SocksOutboundOption},
    vless::{VlessInboundOption, VlessOutboundOption},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboundServiceOption {
    Http(HttpInboundOption),
    Socks(SocksInboundOption),
    Mixed(MixedInboundOption),
    Vless(VlessInboundOption),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundServiceOption {
    Direct,
    Http(HttpOutboundOption),
    Socks(SocksOutboundOption),
    Vless(VlessOutboundOption),
}
