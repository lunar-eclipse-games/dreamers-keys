use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    os::fd::IntoRawFd as _,
    process::{Child, Command},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use common::{
    Error, Result,
    game::character::{Character, CharacterKind},
    message::{
        ReliableMessageFromClient, ReliableMessageFromServer, UnreliableMessageFromClient,
        UnreliableMessageFromServer,
    },
};
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};
use tracing::info;
use uuid::Uuid;

#[derive(Debug)]
struct LocalInstance {
    id: Uuid,
    process: Child,
    client: RenetClient,
    transport: NetcodeClientTransport,
    tx: interprocess::unnamed_pipe::Sender,
    unreliable_message_queue: Vec<UnreliableMessageFromServer>,
    reliable_message_queue: Vec<ReliableMessageFromServer>,
}

#[derive(Debug)]
enum State {
    Inactive,
    LoggedIn {
        character_id: u32,
        active_instance: Uuid,
        connected_instances: Vec<Uuid>,
    },
}

#[derive(Debug)]
pub struct LocalBackend {
    instances: HashMap<Uuid, LocalInstance>,
    home_instances: HashMap<u32, Uuid>,
    characters: Vec<Character>,
    state: State,
}

impl Default for LocalBackend {
    fn default() -> Self {
        LocalBackend::new()
    }
}

impl LocalBackend {
    pub fn new() -> LocalBackend {
        info!("Starting local backend");

        LocalBackend {
            instances: HashMap::new(),
            home_instances: HashMap::new(),
            characters: Vec::new(),
            state: State::Inactive,
        }
    }

    fn create_and_connect_to_instance(&mut self, character_id: u32) -> Result<Uuid> {
        let id = Uuid::now_v7();

        info!("Creating local instance {id}");

        let key = renet_netcode::generate_random_bytes::<32>();

        #[cfg(debug_assertions)]
        let program = "./target/debug/instance";
        #[cfg(not(debug_assertions))]
        let program = "./target/release/instance";

        let (child_tx, rx) = interprocess::unnamed_pipe::pipe()?;
        let tx_handle: std::os::fd::OwnedFd = child_tx.into();
        let tx_handle = tx_handle.into_raw_fd();

        let (tx, child_rx) = interprocess::unnamed_pipe::pipe()?;
        let rx_handle: std::os::fd::OwnedFd = child_rx.into();
        let rx_handle = rx_handle.into_raw_fd();

        let process = Command::new(program)
            .args([
                id.as_simple().to_string(),
                hex::encode(key),
                format!("{tx_handle};{rx_handle}"),
            ])
            .spawn()?;

        let mut reader = BufReader::new(rx);

        let mut server_addr = String::with_capacity(16);

        reader.read_line(&mut server_addr)?;

        let server_addr = SocketAddr::from_str(server_addr.trim())?;

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let connect_token = ConnectToken::generate(
            current_time,
            0,
            30 * 60,
            0,
            30 * 60,
            vec![server_addr],
            None,
            &key,
        )?;

        let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
        let socket = UdpSocket::bind(server_addr)?;

        let client = RenetClient::new(ConnectionConfig::default());

        let transport = NetcodeClientTransport::new(
            current_time,
            ClientAuthentication::Secure { connect_token },
            socket,
        )?;

        self.instances.insert(
            id,
            LocalInstance {
                id,
                process,
                client,
                transport,
                tx,
                reliable_message_queue: Vec::new(),
                unreliable_message_queue: Vec::new(),
            },
        );
        self.home_instances.insert(character_id, id);

        Ok(id)
    }

    pub fn create_character(&mut self, name: &str, kind: CharacterKind) -> Result<Character> {
        if kind == CharacterKind::Normal {
            return Err(Error::InvalidCharacterKind);
        }

        let char = Character {
            account_id: 0,
            character_id: self.characters.len() as u32,
            name: name.into(),
            kind,
        };

        self.characters.push(char.clone());

        Ok(char)
    }

    pub fn enter_game(&mut self, character_id: u32) -> Result<Uuid> {
        let character = self
            .characters
            .get(character_id as usize)
            .ok_or(Error::InvalidCharacterId)?;

        _ = character;

        let home = if let Some(home) = self.home_instances.get(&character_id).copied() {
            home
        } else {
            self.create_and_connect_to_instance(character_id)?
        };

        self.state = State::LoggedIn {
            character_id,
            active_instance: home,
            connected_instances: vec![home],
        };

        Ok(home)
    }

    pub fn pre_update(&mut self, elapsed: std::time::Duration) -> Result<()> {
        for instance in self.instances.values_mut() {
            instance.client.update(elapsed);
            instance.transport.update(elapsed, &mut instance.client)?;

            while let Some(unreliable) = instance.client.receive_message(DefaultChannel::Unreliable)
            {
                let (unreliable, _) =
                    bincode::decode_from_slice(&unreliable, bincode::config::standard())?;
                instance.unreliable_message_queue.push(unreliable);
            }

            while let Some(reliable) = instance
                .client
                .receive_message(DefaultChannel::ReliableUnordered)
            {
                let (reliable, _) =
                    bincode::decode_from_slice(&reliable, bincode::config::standard())?;
                instance.reliable_message_queue.push(reliable);
            }
        }

        Ok(())
    }

    pub fn is_instance_connected(&self, id: Uuid) -> bool {
        if let Some(instance) = self.instances.get(&id) {
            instance.client.is_connected()
        } else {
            false
        }
    }

    pub fn get_unreliable_messages(&self, id: Uuid) -> &[UnreliableMessageFromServer] {
        if let Some(instance) = self.instances.get(&id) {
            &instance.unreliable_message_queue
        } else {
            &[]
        }
    }

    pub fn get_reliable_messages(&self, id: Uuid) -> &[ReliableMessageFromServer] {
        if let Some(instance) = self.instances.get(&id) {
            &instance.reliable_message_queue
        } else {
            &[]
        }
    }

    pub fn send_unreliable_message(
        &mut self,
        id: Uuid,
        message: UnreliableMessageFromClient,
    ) -> Result<()> {
        if let Some(instance) = self.instances.get_mut(&id) {
            instance.client.send_message(
                DefaultChannel::Unreliable,
                bincode::encode_to_vec(message, bincode::config::standard())?,
            );
        }

        Ok(())
    }

    pub fn send_reliable_message(
        &mut self,
        id: Uuid,
        message: ReliableMessageFromClient,
    ) -> Result<()> {
        if let Some(instance) = self.instances.get_mut(&id) {
            instance.client.send_message(
                DefaultChannel::ReliableUnordered,
                bincode::encode_to_vec(message, bincode::config::standard())?,
            );
        }

        Ok(())
    }

    pub fn post_update(&mut self) -> Result<()> {
        for instance in self.instances.values_mut() {
            instance.transport.send_packets(&mut instance.client)?;
            instance.unreliable_message_queue.clear();
            instance.reliable_message_queue.clear();
        }

        Ok(())
    }

    pub fn get_current_character(&self) -> Option<Character> {
        match &self.state {
            State::Inactive => None,
            State::LoggedIn { character_id, .. } => {
                Some(self.characters[*character_id as usize].clone())
            }
        }
    }

    pub fn get_connected_instances(&self) -> &[Uuid] {
        match &self.state {
            State::Inactive => &[],
            State::LoggedIn {
                connected_instances,
                ..
            } => connected_instances,
        }
    }

    pub fn get_current_instance(&self) -> Option<Uuid> {
        match &self.state {
            State::Inactive => None,
            State::LoggedIn {
                active_instance, ..
            } => Some(*active_instance),
        }
    }

    pub fn shutdown(&mut self) -> common::Result<()> {
        for instance in self.instances.values_mut() {
            instance.tx.write_all(b"shutdown\n")?;
            info!("Sent shutdown to {}", instance.id);
        }

        for (_, mut instance) in self.instances.drain() {
            let exit_status = instance.process.wait()?;
            info!("Instance {} exited with status {exit_status}", instance.id);
        }

        Ok(())
    }
}

impl std::ops::Drop for LocalBackend {
    fn drop(&mut self) {
        for instance in self.instances.values_mut() {
            instance.process.kill().unwrap();
        }

        for (_, mut instance) in self.instances.drain() {
            instance.process.wait().unwrap();
        }
    }
}
