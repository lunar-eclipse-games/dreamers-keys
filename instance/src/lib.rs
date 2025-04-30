use std::{
    collections::HashMap,
    fmt::Debug,
    time::{Duration, Instant},
};

use backend::{BackendCommunication, Message};
use common::{
    DT, Entity, Result, Vec2,
    instance::{Instance, LastInputTracker, Player, Position},
    message::{
        NetworkSpawn, OrderedInput, OwnedPlayerSync, PlayerInit, PlayerPositionSync,
        ReliableMessageFromClient, ReliableMessageFromServer, Spawn, TickSync,
        UnreliableMessageFromClient, UnreliableMessageFromServer,
    },
    net_obj::NetworkObject,
    tick::get_unix_millis,
};
use server::Server;
use tick::{TickData, tick};
use tracing::{Level, error, info, instrument, span, warn};
use uuid::Uuid;

// pub mod player;
pub mod backend;
pub mod server;
pub mod tick;

pub fn run(id: Uuid, key: [u8; 32], mut comm: BackendCommunication) -> Result<()> {
    let span = span!(Level::INFO, "instance", %id);
    let _enter = span.enter();

    let server = Server::new(key)?;

    info!("Started server on {}", server.local_address());
    comm.notify_ready(server.local_address())?;

    let mut game = Game::new(id, server);

    let mut start_time = Instant::now();
    let mut accumulator = Duration::ZERO;
    let result: Result<()> = 'main: loop {
        let elapsed = start_time.elapsed();
        accumulator += elapsed;
        start_time = Instant::now();

        if let Err(e) = game.server.update(elapsed) {
            break 'main Err(e);
        }

        while let Some(event) = game.server.get_event() {
            match event {
                renet::ServerEvent::ClientConnected { client_id } => {
                    info!("Client connected: {client_id}");
                    game.message_queues
                        .insert(client_id, MessageQueue::default());
                }
                renet::ServerEvent::ClientDisconnected { client_id, reason } => {
                    info!("Client disconnected: {client_id}, reason: {reason:?}");
                    if let Some(net) = game.client_map.client_to_net_obj.remove(&client_id) {
                        game.client_map.net_obj_to_client.remove(&net);
                        let entity = game.instance.find_network_object(net).unwrap();
                        game.despawn_and_broadcast(entity, net)?;
                    }
                    game.message_queues.remove(&client_id);
                }
            }
        }

        while accumulator >= DT {
            accumulator -= DT;

            if let Err(err) = game.update(DT) {
                break 'main Err(err);
            }
        }

        game.server.send_packets();

        while let Some(msg) = comm.message() {
            match msg {
                Message::Shutdown => {
                    info!("Got shutdown message. Exiting...");
                    break 'main Ok(());
                }
                _ => {}
            }
        }

        std::thread::sleep(DT.saturating_sub(start_time.elapsed()));
    };

    if let Err(err) = result {
        error!("Crashed due to error: {err}");
    } else {
        info!("Exited without error")
    }

    Ok(())
}

#[derive(Default)]
struct MessageQueue {
    reliable: Vec<ReliableMessageFromClient>,
    unreliable: Vec<UnreliableMessageFromClient>,
}

#[derive(Default, Debug)]
struct ClientNetworkObjectMap {
    client_to_net_obj: HashMap<u64, NetworkObject>,
    net_obj_to_client: HashMap<NetworkObject, u64>,
}

#[derive(Default)]
struct ClientInputs {
    inputs: HashMap<NetworkObject, Vec<OrderedInput>>,
}

impl ClientInputs {
    fn push_input(&mut self, net_obj: NetworkObject, input: OrderedInput) {
        self.inputs.entry(net_obj).or_default().push(input);
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
}

pub struct Game {
    instance: Instance,
    server: Server,
    tick: TickData,
    message_queues: HashMap<u64, MessageQueue>,
    client_map: ClientNetworkObjectMap,
    player_spawn_requests: Vec<(Vec2, NetworkObject)>,
    inputs: ClientInputs,
}

impl Debug for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Game").finish_non_exhaustive()
    }
}

impl Game {
    fn new(instance_id: Uuid, server: Server) -> Game {
        Game {
            instance: Instance::new(instance_id),
            server,
            tick: TickData::new(),
            message_queues: HashMap::new(),
            client_map: ClientNetworkObjectMap::default(),
            player_spawn_requests: Vec::new(),
            inputs: ClientInputs::default(),
        }
    }

    fn despawn_and_broadcast(&mut self, entity: Entity, net_obj: NetworkObject) -> Result<()> {
        self.instance.despawn(entity);

        let message = ReliableMessageFromServer::Despawn(net_obj);

        self.server.broadcast_reliable_message(message)?;

        Ok(())
    }

    fn receive_messages(&mut self) -> Result<()> {
        for client_id in self.server.client_ids() {
            let Some(message_queue) = self.message_queues.get_mut(&client_id) else {
                continue;
            };

            while let Some(msg) = self
                .server
                .receive_reliable_message(client_id)
                .transpose()?
            {
                message_queue.reliable.push(msg);
            }

            while let Some(msg) = self
                .server
                .receive_unreliable_message(client_id)
                .transpose()?
            {
                message_queue.unreliable.push(msg);
            }
        }

        Ok(())
    }

    fn read_inputs(&mut self) -> Result<()> {
        for client_id in self.server.client_ids() {
            if let Some(message_queue) = self.message_queues.get(&client_id) {
                for msg in &message_queue.unreliable {
                    if let UnreliableMessageFromClient::Input(ordered_input) = msg {
                        if let Some(net_obj) = self.client_map.client_to_net_obj.get(&client_id) {
                            self.inputs.push_input(*net_obj, ordered_input.clone());
                        } else {
                            warn!("Unknown client_id: {client_id}");
                        }
                    }
                }
            }
        }

        self.inputs.prune(10);

        Ok(())
    }

    fn handle_connections(&mut self) -> Result<()> {
        for (client_id, message_queue) in &self.message_queues {
            for msg in &message_queue.reliable {
                match msg {
                    ReliableMessageFromClient::Connected => {
                        info!("Received connected from {client_id}");
                        if self.client_map.client_to_net_obj.contains_key(client_id) {
                            warn!("connected called more than once");
                            continue;
                        }

                        let net_obj = NetworkObject::new_rand();
                        self.client_map
                            .client_to_net_obj
                            .insert(*client_id, net_obj);
                        self.client_map
                            .net_obj_to_client
                            .insert(net_obj, *client_id);

                        let position = Vec2::zeros();

                        self.player_spawn_requests.push((position, net_obj));

                        let message = ReliableMessageFromServer::PlayerInit(PlayerInit {
                            net_obj,
                            position: position.into(),
                            tick: self.instance.get_tick(),
                        });
                        self.server.send_reliable_message(*client_id, message)?;
                        info!("Sent Player Init");

                        let message = ReliableMessageFromServer::TickSync(TickSync {
                            tick: self.instance.get_tick().get(),
                            unix_millis: get_unix_millis(),
                        });
                        self.server.send_reliable_message(*client_id, message)?;
                        info!("Sent tick sync");
                    }
                    ReliableMessageFromClient::ReadyForUpdates => {
                        info!("Received ready for updates from {client_id}");

                        let tick = self.instance.get_tick();

                        for (_, (net_obj, position, _)) in self
                            .instance
                            .get_world_mut()
                            .query_mut::<(&NetworkObject, &Position, &Player)>()
                        {
                            let net_spawn = NetworkSpawn::Player(position.0.into());
                            let message = ReliableMessageFromServer::Spawn(Spawn {
                                net_obj: *net_obj,
                                net_spawn,
                                tick,
                            });
                            self.server.send_reliable_message(*client_id, message)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn process_player_spawn_requests(&mut self) -> Result<()> {
        for (pos, net_obj) in self.player_spawn_requests.drain(..) {
            self.instance.spawn_player(false, pos, net_obj, None);

            let net_spawn = NetworkSpawn::Player(pos.into());
            let spawn = Spawn {
                net_obj,
                net_spawn,
                tick: self.instance.get_tick(),
            };
            let message = ReliableMessageFromServer::Spawn(spawn);
            self.server.broadcast_reliable_message(message)?;
        }

        Ok(())
    }

    #[instrument]
    fn broadcast_data(&mut self) -> Result<()> {
        for (_, (obj, position, input_tracker)) in
            &mut self
                .instance
                .get_world()
                .query::<(&NetworkObject, &Position, &LastInputTracker)>()
        {
            let Some(client_id) = self.client_map.net_obj_to_client.get(obj) else {
                warn!("No client id for player obj");
                continue;
            };

            let message = UnreliableMessageFromServer::PlayerPositionSync(PlayerPositionSync {
                net_obj: *obj,
                position: position.0.into(),
                tick: self.instance.get_tick(),
            });
            self.server
                .broadcast_unreliable_message_except(*client_id, message)?;

            let message = UnreliableMessageFromServer::OwnedPlayerSync(OwnedPlayerSync {
                net_obj: *obj,
                position: position.0.into(),
                tick: self.instance.get_tick(),
                last_input_order: input_tracker.order,
            });
            self.server.send_unreliable_message(*client_id, message)?;
        }

        Ok(())
    }

    // #[derive(QueryData)]
    // #[query_data(mutable)]
    // struct InputQuery {
    //     entity: Entity,
    //     transform: &'static mut Transform,
    //     net_obj: &'static NetworkObject,
    //     last_input_tracker: &'static mut LastInputTracker,
    //     collider: &'static Collider,
    //     player: &'static mut Player,
    //     rapier_link: &'static RapierContextEntityLink,
    // }

    fn apply_inputs(&mut self, dt: f32) {
        let net_obj_inputs = self.inputs.pop_inputs();

        self.instance.apply_inputs(dt, &net_obj_inputs);
    }

    #[instrument]
    fn update(&mut self, dt: Duration) -> Result<()> {
        tick(self, dt)?;

        self.receive_messages()?;

        self.read_inputs()?;

        self.handle_connections()?;

        self.process_player_spawn_requests()?;

        self.broadcast_data()?;

        self.instance.update(dt)?;

        self.apply_inputs(dt.as_secs_f32());

        self.clear_messages();

        Ok(())
    }

    fn clear_messages(&mut self) {
        for message_queue in self.message_queues.values_mut() {
            message_queue.reliable.clear();
            message_queue.unreliable.clear();
        }
    }
}
