use std::{
    io::{BufRead, BufReader, Cursor},
    net::{SocketAddr, UdpSocket},
    process::{Child, Command, Stdio},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{
    MinimalPlugins,
    app::{App, FixedUpdate, Plugin, Startup},
    core_pipeline::core_2d::Camera2d,
    ecs::{
        event::EventReader,
        schedule::IntoSystemConfigs,
        system::{Commands, ResMut, Resource},
    },
    log,
    utils::HashMap,
};
use bevy_renet::{
    RenetServerPlugin,
    netcode::{
        ConnectToken, NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication,
        ServerConfig,
    },
    renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent},
};
use common::{
    GameLogic,
    manager_message::{ReliableMessageFromClient, ReliableMessageFromServer},
};
use message::{MessagePlugin, ReliableMessageWithSender};
use uuid::Uuid;

mod message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerKind {
    Online,
    Local,
}

pub fn run(kind: ManagerKind) {
    App::new()
        .add_plugins((
            MinimalPlugins,
            bevy::log::LogPlugin::default(),
            Server { kind },
            MessagePlugin,
        ))
        .run();
}

struct Server {
    kind: ManagerKind,
}

impl Plugin for Server {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetServerPlugin);

        let server = RenetServer::new(ConnectionConfig::default());
        app.insert_resource(server);

        app.add_plugins(NetcodeServerPlugin);
        let server_addr = "127.0.0.1:6969".parse().unwrap();
        let socket = UdpSocket::bind(server_addr).unwrap();
        let server_config = ServerConfig {
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(),
            max_clients: 64,
            protocol_id: 0,
            public_addresses: vec![server_addr],
            authentication: match self.kind {
                ManagerKind::Online => ServerAuthentication::Secure {
                    private_key: std::fs::read("manager.key").unwrap().try_into().unwrap(),
                },
                ManagerKind::Local => ServerAuthentication::Unsecure,
            },
        };
        let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
        app.insert_resource(transport);

        app.add_systems(
            FixedUpdate,
            (handle_server_events, handle_connections).in_set(GameLogic::Sync),
        );
        app.init_resource::<Instances>();

        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[derive(Debug)]
struct InstanceData {
    id: Uuid,
    owner: u64,
    key: [u8; 32],
    socket_addr: SocketAddr,
    process: Child,
}

#[derive(Resource, Default, Debug)]
struct Instances {
    id_map: HashMap<Uuid, InstanceData>,
    homes: HashMap<u64, Uuid>,
}

impl Drop for Instances {
    fn drop(&mut self) {
        for (_, instance) in &mut self.id_map {
            instance.process.kill().unwrap();
        }
    }
}

fn handle_server_events(mut server_events: EventReader<ServerEvent>) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                log::info!("Client {client_id} connected");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                log::info!("Client {client_id} disconnected: {reason:?}");
            }
        }
    }
}

fn handle_connections(
    mut server: ResMut<RenetServer>,
    mut reliable_reader: EventReader<ReliableMessageWithSender>,
    mut instances: ResMut<Instances>,
) {
    for ReliableMessageWithSender { client_id, message } in reliable_reader.read() {
        match message {
            ReliableMessageFromClient::RequestHome => {
                let home_data = if let Some(home) = instances.homes.get(client_id).copied() {
                    instances.id_map.get(&home).unwrap()
                } else {
                    let id = Uuid::now_v7();
                    let key = bevy_renet::netcode::generate_random_bytes::<32>();

                    #[cfg(debug_assertions)]
                    let program = "./target/debug/instance-server";
                    #[cfg(not(debug_assertions))]
                    let program = "./target/release/instance-server";

                    let mut process = Command::new(program)
                        .args([id.as_simple().to_string(), hex::encode(key)])
                        .stdout(Stdio::piped())
                        .spawn()
                        .unwrap();

                    let mut reader = BufReader::new(process.stdout.take().unwrap());

                    let mut socket_addr = String::new();

                    reader.read_line(&mut socket_addr).unwrap();

                    let socket_addr = SocketAddr::from_str(socket_addr.trim()).unwrap();

                    let data = InstanceData {
                        id,
                        owner: *client_id,
                        key,
                        socket_addr,
                        process,
                    };

                    instances.id_map.insert(id, data);
                    instances.homes.insert(*client_id, id);

                    instances.id_map.get(&id).unwrap()
                };

                let token = ConnectToken::generate(
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    0,
                    30 * 60,
                    *client_id,
                    30 * 60,
                    vec![home_data.socket_addr.clone()],
                    None,
                    &home_data.key,
                )
                .unwrap();
                let mut token_bytes = Cursor::new(Vec::new());
                token.write(&mut token_bytes).unwrap();

                let bytes = bincode::encode_to_vec(
                    ReliableMessageFromServer::Instance(common::manager_message::InstanceData {
                        id: home_data.id.into_bytes(),
                        token: token_bytes.into_inner(),
                    }),
                    bincode::config::standard(),
                )
                .unwrap();
                server.send_message(*client_id, DefaultChannel::ReliableUnordered, bytes);
            }
            _ => {}
        }
    }
}
