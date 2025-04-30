use std::time::Duration;

use common::{
    Result,
    game::character::{Character, CharacterKind},
    message::{
        ReliableMessageFromClient, ReliableMessageFromServer, UnreliableMessageFromClient,
        UnreliableMessageFromServer,
    },
};
use uuid::Uuid;

pub mod local;

enum BackendInner {
    Local(local::LocalBackend),
}

pub struct BackendConnection(BackendInner);

impl BackendConnection {
    pub fn local() -> BackendConnection {
        BackendConnection(BackendInner::Local(local::LocalBackend::new()))
    }

    pub fn create_character(&mut self, name: &str, kind: CharacterKind) -> Result<Character> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.create_character(name, kind),
        }
    }

    pub fn enter_game(&mut self, character_id: u32) -> Result<Uuid> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.enter_game(character_id),
        }
    }

    pub fn pre_update(&mut self, elapsed: Duration) -> Result<()> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.pre_update(elapsed),
        }
    }

    pub fn is_instance_connected(&self, id: Uuid) -> bool {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.is_instance_connected(id),
        }
    }

    pub fn get_unreliable_messages(&self, id: Uuid) -> &[UnreliableMessageFromServer] {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.get_unreliable_messages(id),
        }
    }

    pub fn get_reliable_messages(&self, id: Uuid) -> &[ReliableMessageFromServer] {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.get_reliable_messages(id),
        }
    }

    pub fn send_unreliable_message(
        &mut self,
        id: Uuid,
        message: UnreliableMessageFromClient,
    ) -> Result<()> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => {
                local_backend.send_unreliable_message(id, message)
            }
        }
    }

    pub fn send_reliable_message(
        &mut self,
        id: Uuid,
        message: ReliableMessageFromClient,
    ) -> Result<()> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.send_reliable_message(id, message),
        }
    }

    pub fn post_update(&mut self) -> Result<()> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.post_update(),
        }
    }

    pub fn get_current_character(&self) -> Option<Character> {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.get_current_character(),
        }
    }

    pub fn get_connected_instances(&self) -> &[Uuid] {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.get_connected_instances(),
        }
    }

    pub fn get_current_instance(&self) -> Option<Uuid> {
        match &self.0 {
            BackendInner::Local(local_backend) => local_backend.get_current_instance(),
        }
    }

    pub fn shutdown(&mut self) -> Result<()> {
        match &mut self.0 {
            BackendInner::Local(local_backend) => local_backend.shutdown(),
        }
    }
}
