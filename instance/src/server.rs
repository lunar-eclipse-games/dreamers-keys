use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

use common::message::{ReliableMessageFromClient, UnreliableMessageFromClient};
use renet::{ConnectionConfig, DefaultChannel, RenetServer};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};

use crate::Result;

#[derive(Debug)]
pub struct Server {
    server: RenetServer,
    transport: NetcodeServerTransport,
    socket_addr: SocketAddr,
}

impl Server {
    pub fn new(private_key: [u8; 32]) -> Result<Server> {
        let server = RenetServer::new(ConnectionConfig::default());

        let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let socket = UdpSocket::bind(server_addr)?;
        let socket_addr = socket.local_addr()?;
        let server_config = ServerConfig {
            current_time: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?,
            max_clients: 256,
            protocol_id: 0,
            public_addresses: vec![socket_addr],
            authentication: ServerAuthentication::Secure { private_key },
        };

        let transport = NetcodeServerTransport::new(server_config, socket)?;

        Ok(Server {
            server,
            transport,
            socket_addr,
        })
    }

    pub fn local_address(&self) -> SocketAddr {
        self.socket_addr
    }

    pub fn update(&mut self, delta: Duration) -> Result<()> {
        self.server.update(delta);
        self.transport.update(delta, &mut self.server)?;

        Ok(())
    }

    pub fn get_event(&mut self) -> Option<renet::ServerEvent> {
        self.server.get_event()
    }

    pub fn client_ids(&self) -> Vec<u64> {
        self.server.clients_id()
    }

    fn decode<T: bincode::Decode<()>>(data: &[u8]) -> Result<T> {
        let (message, _) = bincode::decode_from_slice(data, bincode::config::standard())?;
        Ok(message)
    }

    pub fn receive_reliable_message(
        &mut self,
        client_id: u64,
    ) -> Option<Result<ReliableMessageFromClient>> {
        self.server
            .receive_message(client_id, DefaultChannel::ReliableUnordered)
            .as_deref()
            .map(Self::decode)
    }

    pub fn receive_unreliable_message(
        &mut self,
        client_id: u64,
    ) -> Option<Result<UnreliableMessageFromClient>> {
        self.server
            .receive_message(client_id, DefaultChannel::Unreliable)
            .as_deref()
            .map(Self::decode)
    }

    fn encode<T: bincode::Encode>(message: T) -> Result<Vec<u8>> {
        let bytes = bincode::encode_to_vec(message, bincode::config::standard())?;
        Ok(bytes)
    }

    pub fn broadcast_reliable_message(
        &mut self,
        message: common::message::ReliableMessageFromServer,
    ) -> Result<()> {
        self.server
            .broadcast_message(DefaultChannel::ReliableUnordered, Self::encode(message)?);

        Ok(())
    }

    pub fn broadcast_reliable_message_except(
        &mut self,
        except_id: u64,
        message: common::message::ReliableMessageFromServer,
    ) -> Result<()> {
        self.server.broadcast_message_except(
            except_id,
            DefaultChannel::ReliableUnordered,
            Self::encode(message)?,
        );

        Ok(())
    }

    pub fn send_reliable_message(
        &mut self,
        client_id: u64,
        message: common::message::ReliableMessageFromServer,
    ) -> Result<()> {
        self.server.send_message(
            client_id,
            DefaultChannel::ReliableUnordered,
            Self::encode(message)?,
        );

        Ok(())
    }

    pub fn broadcast_unreliable_message(
        &mut self,
        message: common::message::UnreliableMessageFromServer,
    ) -> Result<()> {
        self.server
            .broadcast_message(DefaultChannel::Unreliable, Self::encode(message)?);

        Ok(())
    }

    pub fn broadcast_unreliable_message_except(
        &mut self,
        except_id: u64,
        message: common::message::UnreliableMessageFromServer,
    ) -> Result<()> {
        self.server.broadcast_message_except(
            except_id,
            DefaultChannel::Unreliable,
            Self::encode(message)?,
        );

        Ok(())
    }

    pub fn send_unreliable_message(
        &mut self,
        client_id: u64,
        message: common::message::UnreliableMessageFromServer,
    ) -> Result<()> {
        self.server.send_message(
            client_id,
            DefaultChannel::Unreliable,
            Self::encode(message)?,
        );

        Ok(())
    }

    pub fn send_packets(&mut self) {
        self.transport.send_packets(&mut self.server);
    }
}
