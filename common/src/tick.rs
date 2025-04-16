use std::time::{SystemTime, UNIX_EPOCH};

use bevy::ecs::system::{ResMut, Resource};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Resource,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
pub struct Tick(u64);

impl Tick {
    pub fn new(tick: u64) -> Self {
        Tick(tick)
    }

    pub fn get(&self) -> u64 {
        self.0
    }
}

pub fn tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
}

pub fn get_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time is before Unix epoch")
        .as_millis()
}
