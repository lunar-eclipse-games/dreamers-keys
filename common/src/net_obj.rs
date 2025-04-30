use std::marker::PhantomData;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::tick::Tick;

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Hash,
)]
pub enum NetworkObject {
    Dynamic(u64),
    Static(u64),
}

impl NetworkObject {
    pub fn new_rand() -> Self {
        Self::Dynamic(rand::random())
    }

    pub fn new_static(id: u64) -> Self {
        Self::Static(id)
    }
}

#[derive(Debug, Clone)]
pub struct LastSyncTracker<T> {
    _component: PhantomData<T>,
    pub last_tick: Tick,
}

impl<T> LastSyncTracker<T> {
    pub fn new(tick: Tick) -> Self {
        Self {
            last_tick: tick,
            _component: PhantomData,
        }
    }

    pub fn should_update(&mut self, tick: Tick) -> bool {
        let should_update = self.last_tick < tick;
        if should_update {
            self.last_tick = tick;
        }
        should_update
    }
}
