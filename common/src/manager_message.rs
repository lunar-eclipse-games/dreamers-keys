use std::io::Cursor;

use bevy::ecs::event::Event;
use bevy_renet::netcode::ConnectToken;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct InstanceData {
    pub id: [u8; 16],
    pub token: Vec<u8>,
}

impl InstanceData {
    pub fn get_token(&self) -> ConnectToken {
        let mut cursor = Cursor::new(&self.token);

        ConnectToken::read(&mut cursor).unwrap()
    }
}

#[derive(Event, Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum ReliableMessageFromServer {
    Instance(InstanceData),
}

#[derive(Event, Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum UnreliableMessageFromServer {}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum ReliableMessageFromClient {
    RequestHome,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[non_exhaustive]
pub enum UnreliableMessageFromClient {}
