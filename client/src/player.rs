use std::collections::{VecDeque, vec_deque};

use bevy::{
    app::{App, FixedUpdate, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        event::{Event, EventReader, EventWriter},
        query::{QueryData, QueryFilter, With, Without},
        schedule::IntoSystemConfigs,
        system::{Commands, Query, Res, ResMut, Resource},
    },
    input::{ButtonInput, keyboard::KeyCode},
    log,
    math::{Vec2, Vec3, Vec3Swizzles},
    time::Time,
    transform::components::Transform,
};
use bevy_rapier2d::{
    geometry::Collider,
    plugin::{RapierContext, RapierContextEntityLink, ReadRapierContext},
};
use common::{
    GameLogic,
    message::{
        NetworkSpawn, OrderedInput, OwnedPlayerSync, PlayerPositionSync, ReliableMessageFromServer,
        UnreliableMessageFromClient, UnreliableMessageFromServer,
    },
    net_obj::{LastSyncTracker, NetworkObject},
    physics::ReadRapierContextExt as _,
    player::{Player, PlayerInput, apply_input},
    tick::Tick,
};

use crate::network::{Client, ReliableMessage, UnreliableMessage};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlayerSpawnRequest>();
        app.insert_resource(InputBuffer::default());
        app.insert_resource(SnapshotHistory::default());
        app.add_systems(
            FixedUpdate,
            (
                read_input.in_set(GameLogic::ReadInput),
                spawn_players.in_set(GameLogic::Spawn),
                spawn_players_from_spawn_requests
                    .in_set(GameLogic::Spawn)
                    .after(spawn_players),
                recv_position_sync.in_set(GameLogic::Sync),
                predict_movement.in_set(GameLogic::Game),
            ),
        );
    }
}

#[derive(Debug, Resource)]
pub struct LocalPlayer(pub NetworkObject);

#[derive(Resource)]
struct Buffer<T> {
    inner: VecDeque<T>,
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Self {
            inner: VecDeque::new(),
        }
    }
}

impl<T> Buffer<T> {
    fn push(&mut self, item: T) {
        self.inner.push_back(item);
    }

    fn prune(&mut self, max_length: usize) {
        while self.inner.len() > max_length {
            self.inner.pop_front();
        }
    }

    fn get_nth_latest(&self, n: usize) -> Option<&T> {
        if n >= self.inner.len() {
            None
        } else {
            self.inner.get(self.inner.len() - 1 - n)
        }
    }

    fn get_latest(&self) -> Option<&T> {
        self.inner.back()
    }

    fn iter(&self) -> vec_deque::Iter<'_, T> {
        self.inner.iter()
    }
}

#[derive(Resource, Default)]
struct InputBuffer {
    buffer: Buffer<OrderedInput>,
    count: u64,
}

impl InputBuffer {
    fn push_input(&mut self, input: PlayerInput) -> u64 {
        self.count += 1;
        self.buffer.push(OrderedInput {
            input,
            order: self.count,
        });
        self.count
    }

    fn get_latest(&self) -> Option<&OrderedInput> {
        self.buffer.get_latest()
    }

    fn get_after(&self, order: u64) -> Vec<OrderedInput> {
        self.buffer
            .iter()
            .filter(|input| input.order > order)
            .cloned()
            .collect()
    }
}

fn read_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut clients: Query<&mut Client>,
    mut input_buffer: ResMut<InputBuffer>,
) {
    let mut local_direction = Vec2::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) {
        local_direction += Vec2::Y;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        local_direction -= Vec2::Y;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        local_direction += Vec2::X;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        local_direction -= Vec2::X;
    }
    let local_direction = local_direction.normalize_or_zero();

    let input = PlayerInput {
        move_direction: local_direction.into(),
    };
    let order = input_buffer.push_input(input.clone());

    for mut client in clients.iter_mut() {
        let message = UnreliableMessageFromClient::Input(OrderedInput {
            input: input.clone(),
            order,
        });
        client.send_unreliable(message);
    }
}

fn spawn_players(
    mut reader: EventReader<ReliableMessage>,
    local_player: Res<LocalPlayer>,
    mut player_spawn_requests: EventWriter<PlayerSpawnRequest>,
) {
    for msg in reader.read() {
        let ReliableMessageFromServer::Spawn(spawn) = &msg.message else {
            continue;
        };

        if spawn.net_obj == local_player.0 {
            continue;
        }

        if let NetworkSpawn::Player(position) = spawn.net_spawn {
            player_spawn_requests.send(PlayerSpawnRequest::Remote {
                position: Vec2::from(position),
                net_obj: spawn.net_obj,
                tick: spawn.tick,
                instance: msg.entity,
            });
        }
    }
}

type SnapshotHistory = Buffer<PlayerSnapshot>;

#[derive(Debug, Clone)]
struct PlayerSnapshot {
    position: Vec2,
}

impl PlayerSnapshot {
    fn is_different(&self, owned_player_sync: &OwnedPlayerSync) -> bool {
        Vec2::from(owned_player_sync.position).distance(self.position) > 0.1
    }
}

#[derive(Component)]
struct LocalPlayerTag;

#[derive(QueryData)]
#[query_data(mutable)]
struct LocalPlayerQuery {
    entity: Entity,
    transform: &'static mut Transform,
    player: &'static mut Player,
    collider: &'static Collider,
    rapier_link: &'static RapierContextEntityLink,
}

#[derive(QueryFilter)]
struct LocalPlayerFilter {
    _filter: (With<Player>, With<LocalPlayerTag>),
}

#[derive(QueryData)]
#[query_data(mutable)]
struct NonLocalPlayers {
    entity: Entity,
    transform: &'static mut Transform,
    net_obj: &'static NetworkObject,
    last_sync_tracker: &'static mut LastSyncTracker<Transform>,
}

#[derive(QueryData)]
#[query_data(mutable)]
struct LocalPlayerQuerySync {
    entity: Entity,
    transform: &'static mut Transform,
    net_obj: &'static NetworkObject,
    last_sync_tracker: &'static mut LastSyncTracker<Transform>,
    player: &'static mut Player,
    collider: &'static Collider,
}

fn recv_position_sync(
    mut reader: EventReader<UnreliableMessage>,
    mut nonlocal_players: Query<NonLocalPlayers, (With<Player>, Without<LocalPlayerTag>)>,
    mut local_player: Query<LocalPlayerQuerySync, LocalPlayerFilter>,
    rapier_context: ReadRapierContext<()>,
    input_buffer: Res<InputBuffer>,
    mut history: ResMut<SnapshotHistory>,
    time: Res<Time>,
) {
    for msg in reader.read() {
        match &msg.message {
            UnreliableMessageFromServer::PlayerPositionSync(position_sync) => {
                sync_nonlocal(&mut nonlocal_players, position_sync);
            }
            UnreliableMessageFromServer::OwnedPlayerSync(owned_player_sync) => {
                let Ok(mut player) = local_player.get_single_mut() else {
                    continue;
                };

                let Ok(rapier_context) = rapier_context.get(msg.entity) else {
                    log::warn!("No rapier context found");
                    continue;
                };

                if *player.net_obj != owned_player_sync.net_obj
                    || !player
                        .last_sync_tracker
                        .should_update(owned_player_sync.tick)
                {
                    continue;
                }

                check_and_rollback(
                    &rapier_context,
                    &mut player,
                    owned_player_sync,
                    &input_buffer,
                    &mut history,
                    &time,
                );
            }
            _ => {}
        }
    }
}

fn sync_nonlocal(
    nonlocal_players: &mut Query<NonLocalPlayers, (With<Player>, Without<LocalPlayerTag>)>,
    position_sync: &PlayerPositionSync,
) {
    for mut player in nonlocal_players.iter_mut() {
        if *player.net_obj != position_sync.net_obj {
            continue;
        }

        if !player.last_sync_tracker.should_update(position_sync.tick) {
            continue;
        }

        player.transform.translation =
            Vec3::new(position_sync.position[0], position_sync.position[1], 0.0);
    }
}

fn check_and_rollback(
    context: &RapierContext,
    player: &mut LocalPlayerQuerySyncItem,
    owned_player_sync: &OwnedPlayerSync,
    input_buffer: &InputBuffer,
    snapshots: &mut SnapshotHistory,
    time: &Time,
) {
    let mut inputs = input_buffer.get_after(owned_player_sync.last_input_order);
    inputs.pop();

    if inputs.is_empty() {
        return;
    }

    let snapshot = snapshots.get_nth_latest(inputs.len());
    let should_reconcile = match snapshot {
        Some(snapshot) => snapshot.is_different(owned_player_sync),
        None => false,
    };

    if !should_reconcile {
        return;
    }

    player.transform.translation = Vec3::new(
        owned_player_sync.position[0],
        owned_player_sync.position[1],
        0.0,
    );

    for input in inputs {
        apply_input(
            context,
            &mut player.transform,
            &input.input,
            player.collider,
            time,
            player.entity,
        );

        snapshots.push(PlayerSnapshot {
            position: player.transform.translation.xy(),
        });
        snapshots.prune(100);
    }
}

fn predict_movement(
    input_buffer: Res<InputBuffer>,
    mut snapshots: ResMut<SnapshotHistory>,
    mut local_player: Query<LocalPlayerQuery, LocalPlayerFilter>,
    time: Res<Time>,
    rapier_context: ReadRapierContext<()>,
) {
    let Ok(mut local_player) = local_player.get_single_mut() else {
        log::warn!("No local player");
        return;
    };

    let Some(input) = input_buffer.get_latest() else {
        log::warn!("No latest input");
        return;
    };

    let Ok(rapier_context) = rapier_context.get(local_player.rapier_link.0) else {
        log::warn!("No rapier context found");
        return;
    };

    apply_input(
        &rapier_context,
        &mut local_player.transform,
        &input.input,
        local_player.collider,
        &time,
        local_player.entity,
    );

    snapshots.push(PlayerSnapshot {
        position: local_player.transform.translation.xy(),
    });
    snapshots.prune(100);
}

#[derive(Event)]
pub enum PlayerSpawnRequest {
    Local {
        position: Vec2,
        net_obj: NetworkObject,
        tick: Tick,
        instance: Entity,
    },
    Remote {
        position: Vec2,
        net_obj: NetworkObject,
        tick: Tick,
        instance: Entity,
    },
}

pub fn spawn_players_from_spawn_requests(
    mut player_spawn_reqs: EventReader<PlayerSpawnRequest>,
    mut commands: Commands,
) {
    for req in player_spawn_reqs.read() {
        match req {
            PlayerSpawnRequest::Local {
                position,
                net_obj: network_object,
                tick,
                instance,
            } => {
                commands.spawn((
                    Player::new(),
                    *network_object,
                    Transform::from_translation(Vec3::new(position.x, position.y, 0.0)),
                    LastSyncTracker::<Transform>::new(*tick),
                    LocalPlayerTag,
                    RapierContextEntityLink(*instance),
                ));
            }
            PlayerSpawnRequest::Remote {
                position,
                net_obj: network_object,
                tick,
                instance,
            } => {
                commands.spawn((
                    Player::new(),
                    *network_object,
                    Transform::from_translation(Vec3::new(position[0], position[1], 0.0)),
                    LastSyncTracker::<Transform>::new(*tick),
                    RapierContextEntityLink(*instance),
                ));
            }
        }
    }
}
