use bevy::{
    app::{FixedUpdate, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        event::{Event, EventReader, EventWriter},
        query::{Added, QueryData, With},
        schedule::IntoSystemConfigs,
        system::{Commands, Query, Res, ResMut, Resource},
    },
    log,
    math::{Vec2, Vec3, Vec3Swizzles},
    time::Time,
    transform::components::Transform,
    utils::HashMap,
};
use bevy_rapier2d::{
    plugin::{RapierContextEntityLink, ReadRapierContext},
    prelude::Collider,
};
use bevy_renet::renet::{ClientId, DefaultChannel, RenetServer};
use common::{
    GameLogic,
    instance_message::{
        NetworkSpawn, OrderedInput, OwnedPlayerSync, PlayerInit, PlayerPositionSync,
        ReliableMessageFromServer, Spawn, UnreliableMessageFromClient, UnreliableMessageFromServer,
    },
    net_obj::NetworkObject,
    physics::ReadRapierContextExt as _,
    player::{Player, apply_input},
    tick::Tick,
};

use crate::{
    ClientNetworkObjectMap, PlayerNeedsInit, PlayerWantsUpdates,
    message::UnreliableMessageWithSender,
};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(ClientInputs::default());
        app.add_event::<PlayerSpawnRequest>();
        app.add_systems(
            FixedUpdate,
            (
                read_inputs.in_set(GameLogic::ReadInput),
                init_players.in_set(GameLogic::Spawn),
                spawn_players_from_spawn_requests
                    .in_set(GameLogic::Spawn)
                    .after(init_players),
                broadcast_player_data.in_set(GameLogic::Sync),
                load_player.in_set(GameLogic::Sync),
                broadcast_player_spawns.in_set(GameLogic::Sync),
                apply_inputs.in_set(GameLogic::Game),
            ),
        );
    }
}

#[derive(Component, Default)]
pub struct LastInputTracker {
    oder: u64,
}

#[derive(Resource, Default)]
pub struct ClientInputs {
    inputs: HashMap<NetworkObject, Vec<OrderedInput>>,
    clients: HashMap<NetworkObject, ClientId>,
}

impl ClientInputs {
    fn push_input(&mut self, net_obj: NetworkObject, input: OrderedInput, client_id: ClientId) {
        self.inputs.entry(net_obj).or_default().push(input);
        self.clients.insert(net_obj, client_id);
    }

    fn pop_inputs(&mut self) -> HashMap<NetworkObject, OrderedInput> {
        let mut inputs = HashMap::new();

        for (obj, ord_inputs) in self.inputs.iter_mut() {
            if let Some((min_index, _)) = ord_inputs
                .iter()
                .enumerate()
                .min_by_key(|(_, input)| input.order)
            {
                let input = ord_inputs.remove(min_index);
                inputs.insert(*obj, input);
            }
        }

        inputs
    }

    fn prune(&mut self, max_length: usize) {
        for (_, ord_inputs) in self.inputs.iter_mut() {
            while ord_inputs.len() > max_length {
                if let Some((min_index, _)) = ord_inputs
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, input)| input.order)
                {
                    ord_inputs.remove(min_index);
                }
            }
        }
    }

    fn _get_client_id(&self, net_obj: &NetworkObject) -> Option<ClientId> {
        self.clients.get(net_obj).cloned()
    }
}

fn read_inputs(
    mut inputs: ResMut<ClientInputs>,
    mut reader: EventReader<UnreliableMessageWithSender>,
    client_netmap: Res<ClientNetworkObjectMap>,
) {
    for UnreliableMessageWithSender { client_id, message } in reader.read() {
        if let UnreliableMessageFromClient::Input(ordered_input) = message {
            if let Some(net_obj) = client_netmap.client_to_net_obj.get(client_id) {
                inputs.push_input(*net_obj, ordered_input.clone(), *client_id);
            } else {
                log::warn!("Unknown clint_id: {client_id}");
            }
        }
    }

    inputs.prune(10);
}

#[derive(Event)]
struct PlayerSpawnRequest {
    position: Vec2,
    net_obj: NetworkObject,
}

fn init_players(
    mut player_init: EventReader<PlayerNeedsInit>,
    mut player_spawn_reqs: EventWriter<PlayerSpawnRequest>,
    mut server: ResMut<RenetServer>,
    tick: Res<Tick>,
) {
    for init in player_init.read() {
        let position = Vec2::ZERO;

        player_spawn_reqs.send(PlayerSpawnRequest {
            position,
            net_obj: init.net_obj,
        });

        log::info!("Sending Player Init");
        let message = ReliableMessageFromServer::PlayerInit(PlayerInit {
            net_obj: init.net_obj,
            position: position.into(),
            tick: *tick,
        });
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        server.send_message(init.client_id, DefaultChannel::ReliableUnordered, bytes);
    }
}

fn spawn_players_from_spawn_requests(
    mut player_spawn_reqs: EventReader<PlayerSpawnRequest>,
    mut commands: Commands,
) {
    for req in player_spawn_reqs.read() {
        commands.spawn((
            Player::new(),
            req.net_obj,
            Transform::from_translation(Vec3::new(req.position.x, req.position.y, 0.0)),
            LastInputTracker::default(),
        ));
    }
}

fn broadcast_player_spawns(
    query: Query<(&NetworkObject, &Transform), Added<Player>>,
    mut server: ResMut<RenetServer>,
    tick: Res<Tick>,
) {
    for (net_obj, transform) in query.iter() {
        let net_spawn = NetworkSpawn::Player([transform.translation.x, transform.translation.y]);
        let spawn = Spawn {
            net_obj: *net_obj,
            net_spawn,
            tick: *tick,
        };
        let message = ReliableMessageFromServer::Spawn(spawn);
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        server.broadcast_message(DefaultChannel::ReliableUnordered, bytes);
    }
}

fn broadcast_player_data(
    player_query: Query<(&NetworkObject, &Transform, &LastInputTracker)>,
    client_netmap: Res<ClientNetworkObjectMap>,
    mut server: ResMut<RenetServer>,
    tick: Res<Tick>,
) {
    for (obj, transform, input_tracker) in player_query.iter() {
        let Some(client_id) = client_netmap.net_obj_to_client.get(obj) else {
            log::warn!("No client id for player obj in broadcast_palyer_data");
            continue;
        };

        let message = UnreliableMessageFromServer::PlayerPositionSync(PlayerPositionSync {
            net_obj: *obj,
            position: [transform.translation.x, transform.translation.y],
            tick: *tick,
        });
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        server.broadcast_message_except(*client_id, DefaultChannel::Unreliable, bytes);

        let message = UnreliableMessageFromServer::OwnedPlayerSync(OwnedPlayerSync {
            net_obj: *obj,
            position: [transform.translation.x, transform.translation.y],
            tick: *tick,
            last_input_order: input_tracker.oder,
        });
        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
        server.send_message(*client_id, DefaultChannel::Unreliable, bytes);
    }
}

fn load_player(
    mut player_wants_updates: EventReader<PlayerWantsUpdates>,
    player_query: Query<(&NetworkObject, &Transform), With<Player>>,
    tick: Res<Tick>,
    mut server: ResMut<RenetServer>,
) {
    for event in player_wants_updates.read() {
        for (net_obj, transform) in player_query.iter() {
            let net_spawn = NetworkSpawn::Player(transform.translation.xy().into());
            let message = ReliableMessageFromServer::Spawn(Spawn {
                net_obj: *net_obj,
                net_spawn,
                tick: *tick,
            });
            let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();
            server.send_message(event.client_id, DefaultChannel::ReliableUnordered, bytes);
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct InputQuery {
    entity: Entity,
    transform: &'static mut Transform,
    net_obj: &'static NetworkObject,
    last_input_tracker: &'static mut LastInputTracker,
    collider: &'static Collider,
    player: &'static mut Player,
    rapier_link: &'static RapierContextEntityLink,
}

fn apply_inputs(
    mut query: Query<InputQuery, With<Player>>,
    mut inputs: ResMut<ClientInputs>,
    rapier_context: ReadRapierContext<()>,
    time: Res<Time>,
) {
    let net_obj_inputs = inputs.pop_inputs();

    for mut item in query.iter_mut() {
        if let Some(input) = net_obj_inputs.get(item.net_obj) {
            let Ok(rapier_context) = rapier_context.get(item.rapier_link.0) else {
                log::warn!("No rapier context found");
                continue;
            };

            apply_input(
                &rapier_context,
                &mut item.transform,
                &input.input,
                item.collider,
                &time,
                item.entity,
            );

            item.last_input_tracker.oder = input.order;
        }
    }
}
