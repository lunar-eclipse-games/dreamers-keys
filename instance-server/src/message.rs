use bevy::{
    app::{App, FixedUpdate, Plugin},
    ecs::{
        event::{Event, EventWriter},
        schedule::IntoSystemConfigs,
        system::ResMut,
    },
    log,
};
use bevy_renet::renet::{DefaultChannel, RenetServer};
use common::{
    GameLogic,
    message::{ReliableMessageFromClient, UnreliableMessageFromClient},
};

#[derive(Debug, Event)]
pub struct ReliableMessageWithSender {
    pub client_id: u64,
    pub message: ReliableMessageFromClient,
}

#[derive(Debug, Event)]
pub struct UnreliableMessageWithSender {
    pub client_id: u64,
    pub message: UnreliableMessageFromClient,
}

pub struct MessagePlugin;

impl Plugin for MessagePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ReliableMessageWithSender>();
        app.add_event::<UnreliableMessageWithSender>();
        app.add_systems(
            FixedUpdate,
            read_messages_from_clients.before(GameLogic::Start),
        );
    }
}

fn read_messages_from_clients(
    mut server: ResMut<RenetServer>,
    mut reliable_writer: EventWriter<ReliableMessageWithSender>,
    mut unreliable_writer: EventWriter<UnreliableMessageWithSender>,
) {
    for client_id in server.clients_id() {
        while let Some(message) =
            server.receive_message(client_id, DefaultChannel::ReliableUnordered)
        {
            if let Ok((message, _)) =
                bincode::decode_from_slice(&message, bincode::config::standard())
            {
                reliable_writer.send(ReliableMessageWithSender { client_id, message });
            } else {
                log::error!("Failed to deserialize message from client");
            }
        }

        while let Some(message) = server.receive_message(client_id, DefaultChannel::Unreliable) {
            if let Ok((message, _)) =
                bincode::decode_from_slice(&message, bincode::config::standard())
            {
                unreliable_writer.send(UnreliableMessageWithSender { client_id, message });
            } else {
                log::error!("Failed to deserialize message from client");
            }
        }
    }
}
