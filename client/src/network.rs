use bevy::{
    app::{App, AppExit, FixedUpdate, Last, Plugin, PostUpdate, PreUpdate},
    ecs::{
        component::Component,
        entity::Entity,
        event::{Event, EventReader, EventWriter},
        query::{Added, With},
        schedule::{IntoSystemConfigs as _, SystemSet},
        system::{Commands, Query, Res, ResMut},
    },
    log,
    math::Vec2,
    state::state::NextState,
    time::Time,
};
use bevy_rapier2d::prelude::RapierContextSimulation;
use bevy_renet::{
    netcode::{NetcodeClientTransport, NetcodeTransportError},
    renet::{DefaultChannel, RenetClient},
};
use common::{GameLogic, instance_message, manager_message};
use uuid::Uuid;

use crate::{
    AppState,
    player::{LocalPlayer, PlayerSpawnRequest},
    tick::get_client_tick,
};

#[derive(Debug, SystemSet, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenetReceive;

#[derive(Debug, SystemSet, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenetSend;

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TransportError>();

        app.add_systems(
            PreUpdate,
            (
                update_clients,
                update_transports.in_set(RenetReceive).after(update_clients),
            ),
        );
        app.add_systems(PostUpdate, send_packets.in_set(RenetSend));
        app.add_systems(Last, disconnect_on_exit);

        app.add_event::<ReliableMessage>();
        app.add_event::<UnreliableMessage>();
        app.add_event::<manager_message::ReliableMessageFromServer>();
        app.add_event::<manager_message::UnreliableMessageFromServer>();
        app.add_systems(
            FixedUpdate,
            read_messages_from_server.before(GameLogic::Start),
        );

        app.add_systems(
            FixedUpdate,
            (load_local, send_ready, load_remote).after(GameLogic::Start),
        );
    }
}

#[derive(Component)]
pub struct Client {
    client: RenetClient,
    transport: NetcodeClientTransport,
}

impl Client {
    pub fn new(client: RenetClient, transport: NetcodeClientTransport) -> Self {
        Client { client, transport }
    }
}

#[derive(Component)]
pub struct Instance(pub Uuid);

#[derive(Component, Default)]
pub struct CurrentInstance;

#[derive(Component, Default)]
pub struct InstanceConnecting;

#[derive(Component, Default)]
pub struct InstanceLocalLoaded;

#[derive(Component, Default)]
pub struct InstanceRemoteLoading {
    set_player_obj: bool,
    tick: bool,
}

impl InstanceRemoteLoading {
    fn all(&self) -> bool {
        self.set_player_obj && self.tick
    }
}

#[derive(Component, Default)]
pub struct InstanceReady;

#[derive(Component, Default)]
pub struct InstanceActive;

impl Client {
    pub fn client(&self) -> &RenetClient {
        &self.client
    }

    pub fn client_mut(&mut self) -> &mut RenetClient {
        &mut self.client
    }

    pub fn send_manager_reliable(&mut self, message: manager_message::ReliableMessageFromClient) {
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        self.client
            .send_message(DefaultChannel::ReliableUnordered, bytes);
    }

    pub fn send_manager_unreliable(
        &mut self,
        message: manager_message::UnreliableMessageFromClient,
    ) {
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        self.client.send_message(DefaultChannel::Unreliable, bytes);
    }

    pub fn send_instance_reliable(&mut self, message: instance_message::ReliableMessageFromClient) {
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        self.client
            .send_message(DefaultChannel::ReliableUnordered, bytes);
    }

    pub fn send_instance_unreliable(
        &mut self,
        message: instance_message::UnreliableMessageFromClient,
    ) {
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        self.client.send_message(DefaultChannel::Unreliable, bytes);
    }
}

#[derive(Event)]
pub struct TransportError {
    pub entity: Entity,
    pub error: NetcodeTransportError,
}

fn update_clients(mut query: Query<&mut Client>, time: Res<Time>) {
    for mut client in query.iter_mut() {
        client.client.update(time.delta())
    }
}

fn update_transports(
    mut query: Query<(Entity, &mut Client)>,
    time: Res<Time>,
    mut transport_errors: EventWriter<TransportError>,
) {
    for (entity, mut client) in query.iter_mut() {
        let Client { client, transport } = &mut *client;
        if let Err(error) = transport.update(time.delta(), client) {
            transport_errors.send(TransportError { entity, error });
        }
    }
}

fn send_packets(
    mut query: Query<(Entity, &mut Client)>,
    mut transport_errors: EventWriter<TransportError>,
) {
    for (entity, mut client) in query.iter_mut() {
        let Client { client, transport } = &mut *client;
        if let Err(error) = transport.send_packets(client) {
            transport_errors.send(TransportError { entity, error });
        }
    }
}

fn disconnect_on_exit(mut query: Query<&mut Client>, exit: EventReader<AppExit>) {
    if !exit.is_empty() {
        for mut client in query.iter_mut() {
            client.transport.disconnect();
        }
    }
}

#[derive(Event)]
pub struct ReliableMessage {
    pub entity: Entity,
    pub message: instance_message::ReliableMessageFromServer,
}

#[derive(Event)]
pub struct UnreliableMessage {
    pub entity: Entity,
    pub message: instance_message::UnreliableMessageFromServer,
}

fn read_messages_from_server(
    mut query: Query<(Entity, &mut Client, Option<&Instance>)>,
    mut reliable_instance_writer: EventWriter<ReliableMessage>,
    mut unreliable_instance_writer: EventWriter<UnreliableMessage>,
    mut reliable_manager_writer: EventWriter<manager_message::ReliableMessageFromServer>,
    mut unreliable_manager_writer: EventWriter<manager_message::UnreliableMessageFromServer>,
) {
    for (entity, mut client, instance) in query.iter_mut() {
        while let Some(message) = client
            .client_mut()
            .receive_message(DefaultChannel::ReliableUnordered)
        {
            if instance.is_some() {
                if let Ok((message, _)) = bincode::decode_from_slice::<
                    instance_message::ReliableMessageFromServer,
                    bincode::config::Configuration,
                >(&message, bincode::config::standard())
                {
                    reliable_instance_writer.send(ReliableMessage { entity, message });
                } else {
                    log::error!("Failed to deserialize message from instance server");
                }
            } else {
                if let Ok((message, _)) = bincode::decode_from_slice::<
                    manager_message::ReliableMessageFromServer,
                    bincode::config::Configuration,
                >(&message, bincode::config::standard())
                {
                    reliable_manager_writer.send(message);
                } else {
                    log::error!("Failed to deserialize message from manager server");
                }
            }
        }

        while let Some(message) = client
            .client_mut()
            .receive_message(DefaultChannel::Unreliable)
        {
            if instance.is_some() {
                if let Ok((message, _)) = bincode::decode_from_slice::<
                    instance_message::UnreliableMessageFromServer,
                    bincode::config::Configuration,
                >(&message, bincode::config::standard())
                {
                    unreliable_instance_writer.send(UnreliableMessage { entity, message });
                } else {
                    log::error!("Failed to deserialize message from instance server");
                }
            } else {
                if let Ok((message, _)) = bincode::decode_from_slice::<
                    manager_message::UnreliableMessageFromServer,
                    bincode::config::Configuration,
                >(&message, bincode::config::standard())
                {
                    unreliable_manager_writer.send(message);
                } else {
                    log::error!("Failed to deserialize message from manager server");
                }
            }
        }
    }
}

fn load_local(
    mut commands: Commands,
    clients: Query<(Entity, &Instance), (With<Client>, Added<InstanceConnecting>)>,
) {
    for (client, Instance(id)) in clients.iter() {
        log::info!("load_local: {id}");

        commands
            .entity(client)
            .remove::<InstanceConnecting>()
            .insert(RapierContextSimulation::default())
            .insert(InstanceLocalLoaded);
    }
}

fn send_ready(
    mut commands: Commands,
    mut clients: Query<(Entity, &Instance, &mut Client), With<InstanceLocalLoaded>>,
) {
    for (entity, Instance(id), mut client) in clients.iter_mut() {
        log::info!("send_ready: {id}");

        if client.client().is_connected() {
            client.send_instance_reliable(instance_message::ReliableMessageFromClient::Connected);
            log::info!("Connected.");
            commands
                .entity(entity)
                .remove::<InstanceLocalLoaded>()
                .insert(InstanceRemoteLoading::default());
        }
    }
}

fn load_remote(
    mut commands: Commands,
    mut reliable_reader: EventReader<ReliableMessage>,
    mut player_spawn_requests: EventWriter<PlayerSpawnRequest>,
    mut clients: Query<(Entity, &Instance, &mut Client, &mut InstanceRemoteLoading)>,
    active_instance: Query<Entity, With<InstanceActive>>,
    current_instance: Query<Entity, With<CurrentInstance>>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    for msg in reliable_reader.read() {
        for (instance, Instance(id), mut client, mut state) in clients.iter_mut() {
            if msg.entity != instance {
                continue;
            }

            log::info!("load_remote: {id}");

            match &msg.message {
                instance_message::ReliableMessageFromServer::PlayerInit(player_info) => {
                    log::info!("Got init");
                    commands.insert_resource(LocalPlayer(player_info.net_obj));
                    player_spawn_requests.send(PlayerSpawnRequest::Local {
                        position: Vec2::from(player_info.position),
                        net_obj: player_info.net_obj,
                        tick: player_info.tick,
                        instance,
                    });
                    state.set_player_obj = true;
                }
                instance_message::ReliableMessageFromServer::TickSync(tick_sync) => {
                    log::info!("Got tick sync");
                    let tick = get_client_tick(tick_sync.tick, tick_sync.unix_millis);
                    commands.insert_resource(tick);
                    state.tick = true;
                }
                _ => {}
            }

            if state.all() {
                log::info!("Loaded Remote");
                client.send_instance_reliable(
                    instance_message::ReliableMessageFromClient::ReadyForUpdates,
                );
                log::info!("Sent Ready for Updates");
                if current_instance.get(instance).is_ok() || active_instance.is_empty() {
                    commands
                        .entity(instance)
                        .remove::<InstanceRemoteLoading>()
                        .insert(InstanceActive);
                    app_state.set(AppState::InGame);
                    log::info!("Set active Instance");
                } else {
                    commands
                        .entity(instance)
                        .remove::<InstanceRemoteLoading>()
                        .insert(InstanceReady);
                    log::info!("Set instance ready");
                }
            }
        }
    }
}
