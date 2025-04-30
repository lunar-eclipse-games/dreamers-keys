use std::{
    collections::{VecDeque, vec_deque},
    time::Duration,
};

use common::{
    Entity, Result, Vec2,
    instance::{Instance, LocalPlayer, Player, Position},
    message::{
        NetworkSpawn, OrderedInput, OwnedPlayerSync, PlayerPositionSync, ReliableMessageFromClient,
        ReliableMessageFromServer, UnreliableMessageFromClient, UnreliableMessageFromServer,
    },
    net_obj::{LastSyncTracker, NetworkObject},
    player::PlayerInput,
    tick::{Tick, get_unix_millis},
};
use tracing::{info, warn};

use crate::{KeyboardState, backend::BackendConnection};

pub struct InstanceData {
    instance: Instance,
    state: InstanceState,
    local_player: Option<(NetworkObject, Entity)>,
    input_buffer: InputBuffer,
    player_history: SnapshotHistory,
}

fn get_client_tick(server_tick: u64, server_unix_millis: u128) -> Tick {
    let client_unix_millis = get_unix_millis();

    let elapsed_millis = client_unix_millis.saturating_sub(server_unix_millis);

    const MILLIS_PER_TICK: u128 = 1000 / 60;

    let elapsed_ticks = elapsed_millis / MILLIS_PER_TICK;

    Tick::new(server_tick + elapsed_ticks as u64)
}

impl InstanceData {
    pub fn new(instance: Instance) -> InstanceData {
        InstanceData {
            instance,
            state: InstanceState::Connecting,
            local_player: None,
            input_buffer: InputBuffer::default(),
            player_history: SnapshotHistory::default(),
        }
    }

    fn recv_tick_update(&mut self, backend: &mut BackendConnection) {
        if self.state == InstanceState::Done {
            for msg in backend.get_reliable_messages(self.instance.get_id()) {
                if let ReliableMessageFromServer::TickSync(sync) = msg {
                    let next_tick = get_client_tick(sync.tick, sync.unix_millis);
                    self.instance.set_tick(next_tick);
                }
            }
        }
    }

    fn read_input(&mut self, backend: &mut BackendConnection, kb: &KeyboardState) -> Result<()> {
        let mut local_direction = Vec2::zeros();
        if kb.is_pressed(glfw::Key::W, None) {
            local_direction += Vec2::y();
        }
        if kb.is_pressed(glfw::Key::S, None) {
            local_direction -= Vec2::y();
        }
        if kb.is_pressed(glfw::Key::D, None) {
            local_direction += Vec2::x();
        }
        if kb.is_pressed(glfw::Key::A, None) {
            local_direction -= Vec2::x();
        }
        let local_direction = if local_direction == Vec2::zeros() {
            local_direction
        } else {
            local_direction.normalize()
        };

        let input = PlayerInput {
            move_direction: local_direction.into(),
        };
        let order = self.input_buffer.push_input(input.clone());

        let message = UnreliableMessageFromClient::Input(OrderedInput {
            input: input.clone(),
            order,
        });
        backend.send_unreliable_message(self.instance.get_id(), message)?;

        Ok(())
    }

    fn spawn(&mut self, backend: &mut BackendConnection) -> Result<()> {
        for msg in backend.get_reliable_messages(self.instance.get_id()) {
            let ReliableMessageFromServer::Spawn(spawn) = msg else {
                continue;
            };

            if self.local_player.map(|x| x.0) == Some(spawn.net_obj) {
                continue;
            }

            if let NetworkSpawn::Player(position) = spawn.net_spawn {
                self.instance
                    .spawn_player(false, position.into(), spawn.net_obj, Some(spawn.tick));
            }
        }

        Ok(())
    }

    fn sync_nonlocal(&mut self, position_sync: &PlayerPositionSync) {
        for (_, (position, net_obj, last_sync_tracker)) in self
            .instance
            .get_world_mut()
            .query_mut::<(
                &mut Position,
                &NetworkObject,
                &mut LastSyncTracker<Position>,
            )>()
            .with::<&Player>()
            .without::<&LocalPlayer>()
        {
            if *net_obj != position_sync.net_obj {
                continue;
            }

            if !last_sync_tracker.should_update(position_sync.tick) {
                continue;
            }

            position.0 = Vec2::new(position_sync.position[0], position_sync.position[1]);
        }
    }

    fn recv_position_sync(&mut self, backend: &mut BackendConnection, dt: Duration) {
        for msg in backend.get_unreliable_messages(self.instance.get_id()) {
            match msg {
                UnreliableMessageFromServer::PlayerPositionSync(position_sync) => {
                    self.sync_nonlocal(position_sync);
                }
                UnreliableMessageFromServer::OwnedPlayerSync(owned_player_sync) => {
                    let Some((player, (net_obj, last_sync_tracker))) = self
                        .instance
                        .get_world_mut()
                        .query_mut::<(&NetworkObject, &mut LastSyncTracker<Position>)>()
                        .with::<&LocalPlayer>()
                        .into_iter()
                        .next()
                    else {
                        continue;
                    };

                    if *net_obj != owned_player_sync.net_obj
                        || !last_sync_tracker.should_update(owned_player_sync.tick)
                    {
                        continue;
                    }

                    let mut inputs = self
                        .input_buffer
                        .get_after(owned_player_sync.last_input_order);
                    inputs.pop();

                    if inputs.is_empty() {
                        continue;
                    }

                    let snapshot = self.player_history.get_nth_latest(inputs.len());
                    let should_reconcile = match snapshot {
                        Some(snapshot) => snapshot.is_different(owned_player_sync),
                        None => false,
                    };

                    if !should_reconcile {
                        continue;
                    }

                    self.instance.check_and_rollback(
                        player,
                        owned_player_sync,
                        dt.as_secs_f32(),
                        inputs,
                        |pos| {
                            self.player_history.push(PlayerSnapshot { position: pos });
                            self.player_history.prune(100);
                        },
                    );
                }
                _ => {}
            }
        }
    }

    fn predict_movement(&mut self, dt: Duration) {
        let Some((_, local_player)) = self.local_player else {
            warn!("No local player");
            return;
        };

        let Some(input) = self.input_buffer.get_latest() else {
            warn!("No latest input");
            return;
        };

        let Some(new_position) =
            self.instance
                .apply_input(local_player, &input.input, dt.as_secs_f32())
        else {
            warn!("No valid local player");
            return;
        };

        self.player_history.push(PlayerSnapshot {
            position: new_position,
        });
        self.player_history.prune(100);
    }

    pub fn update(
        &mut self,
        backend: &mut BackendConnection,
        kb: &KeyboardState,
        dt: Duration,
    ) -> Result<()> {
        let id = self.instance.get_id();

        self.instance.update_tick();

        self.recv_tick_update(backend);

        let next_state = match &mut self.state {
            InstanceState::Connecting => {
                // start loading

                // check if done: then switch state

                Some(InstanceState::LocalLoaded)
            }
            InstanceState::LocalLoaded => {
                if backend.is_instance_connected(id) {
                    backend.send_reliable_message(id, ReliableMessageFromClient::Connected)?;
                    info!("Instance {id} Connected.");
                    Some(InstanceState::LoadRemote(LoadRemoteState::default()))
                } else {
                    None
                }
            }
            InstanceState::LoadRemote(state) => {
                for msg in backend.get_reliable_messages(id) {
                    match msg {
                        ReliableMessageFromServer::PlayerInit(player_info) => {
                            info!("Got init");
                            let entity = self.instance.spawn_player(
                                true,
                                player_info.position.into(),
                                player_info.net_obj,
                                Some(player_info.tick),
                            );
                            self.local_player = Some((player_info.net_obj, entity));
                            state.set_player_obj = true;
                        }
                        ReliableMessageFromServer::TickSync(tick_sync) => {
                            info!("Got tick sync");
                            let tick = get_client_tick(tick_sync.tick, tick_sync.unix_millis);
                            self.instance.set_tick(tick);
                            state.tick = true;
                        }
                        _ => {}
                    }
                }

                if state.all() {
                    info!("Loaded Remote");
                    backend
                        .send_reliable_message(id, ReliableMessageFromClient::ReadyForUpdates)?;
                    info!("Sent Ready for Updates");
                    Some(InstanceState::Done)
                } else {
                    None
                }
            }
            InstanceState::Done => {
                self.read_input(backend, kb)?;

                self.spawn(backend)?;

                self.recv_position_sync(backend, dt);

                self.predict_movement(dt);

                None
            }
        };

        if let Some(next_state) = next_state {
            self.state = next_state;
        }

        self.instance.update(dt)?;

        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LoadRemoteState {
    set_player_obj: bool,
    tick: bool,
}

impl LoadRemoteState {
    fn all(&self) -> bool {
        self.set_player_obj && self.tick
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum InstanceState {
    Connecting,
    LocalLoaded,
    LoadRemote(LoadRemoteState),
    Done,
}

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

#[derive(Default)]
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

type SnapshotHistory = Buffer<PlayerSnapshot>;

#[derive(Debug, Clone)]
struct PlayerSnapshot {
    position: Vec2,
}

impl PlayerSnapshot {
    fn is_different(&self, owned_player_sync: &OwnedPlayerSync) -> bool {
        Vec2::from(owned_player_sync.position).metric_distance(&self.position) > 0.1
    }
}
