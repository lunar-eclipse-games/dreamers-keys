use bevy::{app::{App, FixedUpdate, Plugin}, ecs::{event::EventReader, schedule::IntoSystemConfigs as _, system::ResMut}};
use common::{message::ReliableMessageFromServer, tick::{get_unix_millis, tick, Tick}, GameLogic};

use crate::network::ReliableMessage;

pub struct TickPlugin;

impl Plugin for TickPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, tick.in_set(GameLogic::Start));

        app.add_systems(
            FixedUpdate,
            recv_tick_update.in_set(GameLogic::Start).after(tick),
        );
    }
}

fn recv_tick_update(mut reader: EventReader<ReliableMessage>, mut curr_tick: ResMut<Tick>) {
    for msg in reader.read() {
        if let ReliableMessageFromServer::TickSync(sync) = &msg.message {
            let next_tick = get_client_tick(sync.tick, sync.unix_millis);
            *curr_tick = next_tick;
        }
    }
}

pub fn get_client_tick(server_tick: u64, server_unix_millis: u128) -> Tick {
    let client_unix_millis = get_unix_millis();

    let elapsed_millis = client_unix_millis.saturating_sub(server_unix_millis);

    const MILLIS_PER_TICK: u128 = 1000 / 60;

    let elapsed_ticks = elapsed_millis / MILLIS_PER_TICK;

    Tick::new(server_tick + elapsed_ticks as u64)
}