use std::{borrow::Cow, collections::HashMap, str::FromStr};

use tokio::io::{AsyncRead, AsyncWrite, BufStream};
use uuid::Uuid;

use crate::{
    address::NetworkType, InboundError, InboundPacket, InboundResult, InboundServiceTrait,
};

use super::{
    option::VlessInboundOption,
    protocol::{Request, Response, COMMAND_TCP, COMMAND_UDP},
    VlessError,
};

#[derive(Debug)]
pub struct VlessInbound {
    users: HashMap<uuid::Uuid, String>,
}

impl VlessInbound {
    pub fn add_user(&mut self, uuid: uuid::Uuid, user: String) {
        self.users.insert(uuid, user);
    }

    pub fn init(option: VlessInboundOption) -> InboundResult<Self> {
        let mut users = HashMap::new();

        for user in option.users {
            let uuid =
                Uuid::from_str(&user.uuid).map_err(|e| InboundError::Option(e.to_string()))?;
            users.insert(uuid, user.user);
        }

        Ok(Self { users })
    }
}

impl<S> InboundServiceTrait<S> for VlessInbound
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Stream = BufStream<S>;

    async fn handshake(&self, stream: S) -> InboundResult<(Self::Stream, InboundPacket)> {
        let mut stream = BufStream::new(stream);
        let request = Request::read(&mut stream)
            .await
            .map_err(|e| InboundError::Handshake(e.into()))?;

        let user = self
            .users
            .get(&request.uuid)
            .ok_or(InboundError::Handshake(
                VlessError::InvalidUuid(request.uuid.to_string()).into(),
            ))?;

        let pac = match request.command {
            COMMAND_TCP => {
                let dest = match request.destination {
                    Some(d) => d,
                    None => return Err(InboundError::Handshake(VlessError::NoDestination.into())),
                };
                InboundPacket {
                    typ: NetworkType::Tcp,
                    dest,
                    detail: Cow::Borrowed(user),
                }
            }
            COMMAND_UDP => {
                let dest = match request.destination {
                    Some(d) => d,
                    None => return Err(InboundError::Handshake(VlessError::NoDestination.into())),
                };
                InboundPacket {
                    typ: NetworkType::Udp,
                    dest,
                    detail: Cow::Borrowed(user),
                }
            }
            //COMMAND_MUX => unimplemented!(),
            _ => {
                return Err(InboundError::Handshake(
                    VlessError::InvalidCommand(request.command).into(),
                ))
            }
        };

        let _ = Response::default()
            .write(&mut stream, None)
            .await
            .map_err(|e| InboundError::Handshake(e.into()))?;

        Ok((stream, pac))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::vless::option::VlessUserOption;

    use super::*;

    #[tokio::test]
    async fn test_vless_inbound() {
        let buf: Vec<u8> = vec![
            0, 252, 66, 254, 52, 226, 103, 76, 105, 136, 97, 43, 196, 25, 5, 117, 25, 0, 1, 34,
            184, 1, 127, 0, 0, 1, 116, 101, 115, 116,
        ];

        let s = Cursor::new(buf);

        let opt = VlessInboundOption {
            users: vec![VlessUserOption {
                user: "test".into(),
                uuid: "fc42fe34-e267-4c69-8861-2bc419057519".into(),
            }],
        };

        let vi = VlessInbound::init(opt).unwrap();

        let result = vi.handshake(s).await.unwrap();

        println!("{:?}", result)
    }
}
