//! Socks service

pub mod option;
pub use option::{SocksInboundOption, SocksOutboundOption};

pub mod inbound;
pub use inbound::SocksInbound;

pub mod outbound;
pub use outbound::SocksOutbound;

pub mod protocol;
pub use protocol::SocksError;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    use crate::{
        address::NetworkType, socks::option::SocksAuthOption, InboundServiceTrait, OutboundPacket,
        OutboundServiceTrait, ServiceAddress,
    };

    use super::*;

    #[tokio::test]
    async fn test_socks_svc() {
        let (mut s1, mut s2) = duplex(4096);

        tokio::spawn(async move {
            let svc_opt = SocksInboundOption {
                auth: vec![
                    SocksAuthOption::Username {
                        user: "test".into(),
                        pass: "test".into(),
                    },
                    SocksAuthOption::Socks4("test".into()),
                ],
            };

            let socks_in = SocksInbound::init(svc_opt).unwrap();

            loop {
                let (mut s, p) = socks_in.handshake(&mut s2).await.unwrap();
                println!("{:?}", p);
                let mut buf = [0u8; 5];
                let n = s.read(&mut buf).await.unwrap();
                assert_eq!(n, 5);
                assert_eq!(&buf, "hello".as_bytes());
                let _ = s.write("byebye".as_bytes()).await.unwrap();
                let _ = s.flush().await.unwrap();
            }
        });

        let socks_opt_v5 = SocksOutboundOption {
            version: 5,
            auth: SocksAuthOption::Username {
                user: "test".into(),
                pass: "test".into(),
            },
        };

        let socks_opt_v4 = SocksOutboundOption {
            version: 4,
            auth: SocksAuthOption::Socks4("test".into()),
        };

        let in_pac = OutboundPacket {
            typ: NetworkType::Tcp,
            dest: ServiceAddress {
                addr: "127.0.0.1".into(),
                port: 7890,
            },
        };

        tokio::time::sleep(Duration::from_millis(100)).await;

        let out_v4 = SocksOutbound::init(socks_opt_v4).unwrap();
        let s = out_v4.handshake(&mut s1, in_pac.clone()).await.unwrap();
        let _ = s.write("hello".as_bytes()).await.unwrap();
        let _ = s.flush().await.unwrap();
        let mut buf = [0u8; 6];
        let n = s.read(&mut buf).await.unwrap();
        assert_eq!(n, 6);
        assert_eq!(&buf, "byebye".as_bytes());

        let out_v5 = SocksOutbound::init(socks_opt_v5).unwrap();
        let s = out_v5.handshake(&mut s1, in_pac.clone()).await.unwrap();
        let _ = s.write("hello".as_bytes()).await.unwrap();
        let _ = s.flush().await.unwrap();
        let mut buf = [0u8; 6];
        let n = s.read(&mut buf).await.unwrap();
        assert_eq!(n, 6);
        assert_eq!(&buf, "byebye".as_bytes());
    }
}
