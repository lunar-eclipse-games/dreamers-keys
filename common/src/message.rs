use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{net_obj::NetworkObject, player::PlayerInput, tick::Tick};

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct TickSync {
    pub tick: u64,
    pub unix_millis: u128,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum NetworkSpawn {
    Player([f32; 2]),
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct Spawn {
    pub net_obj: NetworkObject,
    pub net_spawn: NetworkSpawn,
    pub tick: Tick,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerInit {
    pub net_obj: NetworkObject,
    pub position: [f32; 2],
    pub tick: Tick,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ReliableMessageFromServer {
    InstanceId([u8; 16]),
    TickSync(TickSync),
    Spawn(Spawn),
    PlayerInit(PlayerInit),
    Despawn(NetworkObject),
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerPositionSync {
    pub net_obj: NetworkObject,
    pub position: [f32; 2],
    pub tick: Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OwnedPlayerSync {
    pub net_obj: NetworkObject,
    pub position: [f32; 2],
    pub tick: Tick,
    pub last_input_order: u64,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum UnreliableMessageFromServer {
    PlayerPositionSync(PlayerPositionSync),
    OwnedPlayerSync(OwnedPlayerSync),
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum ReliableMessageFromClient {
    Connected,
    ReadyForUpdates,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct OrderedInput {
    pub input: PlayerInput,
    pub order: u64,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum UnreliableMessageFromClient {
    Input(OrderedInput),
}
