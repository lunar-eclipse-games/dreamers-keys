use std::time::Duration;

use common::{
    Result,
    message::{ReliableMessageFromServer, TickSync},
    tick::get_unix_millis,
};

use crate::Game;

#[derive(Debug)]
pub struct TickData {
    broadcast_timer: Duration,
}

impl TickData {
    pub fn new() -> TickData {
        TickData {
            broadcast_timer: Duration::ZERO,
        }
    }
}

const TICK_BROADCAST_INTERVAL: Duration = Duration::from_secs(10);

pub fn tick(game: &mut Game, duration: Duration) -> Result<()> {
    game.instance.update_tick();

    game.tick.broadcast_timer += duration;

    while game.tick.broadcast_timer >= TICK_BROADCAST_INTERVAL {
        game.tick.broadcast_timer -= TICK_BROADCAST_INTERVAL;

        game.server
            .broadcast_reliable_message(ReliableMessageFromServer::TickSync(TickSync {
                tick: game.instance.get_tick().get(),
                unix_millis: get_unix_millis(),
            }))?;
    }

    Ok(())
}
