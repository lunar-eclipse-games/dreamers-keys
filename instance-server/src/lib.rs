use std::{net::UdpSocket, time::SystemTime};

use bevy::{
    DefaultPlugins,
    app::{App, FixedUpdate, Plugin, PluginGroup as _, Startup},
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::{Event, EventReader, EventWriter},
        query::With,
        schedule::IntoSystemConfigs,
        system::{Commands, Query, Res, ResMut, Resource},
    },
    hierarchy::DespawnRecursiveExt,
    log,
    utils::HashMap,
    window::{Window, WindowPlugin},
};
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use bevy_renet::{
    RenetServerPlugin,
    netcode::{NetcodeServerPlugin, NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent},
};
use common::{
    GameLogic, GameLogicPlugin,
    message::{ReliableMessageFromClient, ReliableMessageFromServer, TickSync},
    net_obj::NetworkObject,
    player::Player,
    tick::{Tick, get_unix_millis},
};
use message::ReliableMessageWithSender;
use uuid::Uuid;

pub mod message;
pub mod player;
pub mod tick;

pub fn run() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Server".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            GameLogicPlugin::new(|| true),
            Server,
        ))
        .add_plugins((
            tick::TickPlugin,
            message::MessagePlugin,
            player::PlayerPlugin,
        ))
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::default(),
            RapierDebugRenderPlugin::default(),
        ))
        .insert_resource(InstanceId(Uuid::now_v7()))
        .run();
}

#[derive(Resource)]
struct InstanceId(Uuid);

struct Server;

impl Plugin for Server {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetServerPlugin);
        app.insert_resource(ClientNetworkObjectMap::default());

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
            authentication: ServerAuthentication::Unsecure,
        };
        let transport = NetcodeServerTransport::new(server_config, socket).unwrap();
        app.insert_resource(transport);

        app.add_event::<PlayerWantsUpdates>();
        app.add_event::<PlayerNeedsInit>();

        app.add_systems(
            FixedUpdate,
            (handle_server_events, handle_connections).in_set(GameLogic::Sync),
        );

        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[derive(Resource, Default, Debug)]
struct ClientNetworkObjectMap {
    client_to_net_obj: HashMap<ClientId, NetworkObject>,
    net_obj_to_client: HashMap<NetworkObject, ClientId>,
}

fn handle_server_events(
    mut server: ResMut<RenetServer>,
    mut commands: Commands,
    mut server_events: EventReader<ServerEvent>,
    mut client_map: ResMut<ClientNetworkObjectMap>,
    query: Query<(Entity, &NetworkObject), With<Player>>,
) {
    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                log::info!("Client {client_id} connected");
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                log::info!("Client {client_id} disconnected: {reason:?}");
                if let Some(net_obj) = client_map.client_to_net_obj.remove(client_id) {
                    client_map.net_obj_to_client.remove(&net_obj);
                    for (entity, obj) in query.iter() {
                        if *obj == net_obj {
                            despawn_recursive_and_broadcast(
                                &mut server,
                                &mut commands,
                                entity,
                                net_obj,
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn despawn_recursive_and_broadcast(
    server: &mut RenetServer,
    commands: &mut Commands,
    entity: Entity,
    net_obj: NetworkObject,
) {
    let message = ReliableMessageFromServer::Despawn(net_obj);
    let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
    server.broadcast_message(DefaultChannel::ReliableUnordered, bytes);
    commands.entity(entity).despawn_recursive();
}

#[derive(Event)]
struct PlayerWantsUpdates {
    pub client_id: ClientId,
}

#[derive(Event)]
struct PlayerNeedsInit {
    pub client_id: ClientId,
    pub net_obj: NetworkObject,
}

fn handle_connections(
    mut server: ResMut<RenetServer>,
    mut reliable_reader: EventReader<ReliableMessageWithSender>,
    mut client_map: ResMut<ClientNetworkObjectMap>,
    tick: Res<Tick>,
    mut player_inits: EventWriter<PlayerNeedsInit>,
    mut player_updates: EventWriter<PlayerWantsUpdates>,
    instance_id: Res<InstanceId>,
) {
    for ReliableMessageWithSender { client_id, message } in reliable_reader.read() {
        match message {
            ReliableMessageFromClient::Connected => {
                log::info!("Received connected from {client_id}");
                if client_map.client_to_net_obj.contains_key(client_id) {
                    log::warn!("connected called more than once");
                    continue;
                }

                let net_obj = NetworkObject::new_rand();
                client_map.client_to_net_obj.insert(*client_id, net_obj);
                client_map.net_obj_to_client.insert(net_obj, *client_id);

                let message = ReliableMessageFromServer::InstanceId(instance_id.0.into_bytes());
                let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
                server.send_message(*client_id, DefaultChannel::ReliableUnordered, bytes);
                log::info!("Sent instance id");

                player_inits.send(PlayerNeedsInit {
                    client_id: *client_id,
                    net_obj,
                });

                let message = ReliableMessageFromServer::TickSync(TickSync {
                    tick: tick.get(),
                    unix_millis: get_unix_millis(),
                });
                let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
                server.send_message(*client_id, DefaultChannel::ReliableUnordered, bytes);
                log::info!("Sent tick sync");
            }
            ReliableMessageFromClient::ReadyForUpdates => {
                player_updates.send(PlayerWantsUpdates {
                    client_id: *client_id,
                });
            }
            _ => {}
        }
    }
}
