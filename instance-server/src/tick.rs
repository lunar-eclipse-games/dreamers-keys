use std::time::Duration;

use bevy::{
    app::{App, FixedUpdate, Plugin},
    ecs::{
        schedule::IntoSystemConfigs as _,
        system::{Res, ResMut, Resource},
    },
    time::{Time, Timer, TimerMode},
};
use bevy_renet::renet::{DefaultChannel, RenetServer};
use common::{
    GameLogic,
    message::{ReliableMessageFromServer, TickSync},
    tick::{Tick, get_unix_millis, tick},
};

#[derive(Resource, Debug)]
pub struct TickBroadcastTimer(Timer);

impl Default for TickBroadcastTimer {
    fn default() -> Self {
        Self(Timer::new(Duration::from_secs(10), TimerMode::Repeating))
    }
}

pub struct TickPlugin;

impl Plugin for TickPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, tick.in_set(GameLogic::Start));
        app.insert_resource(Tick::new(0));
        app.insert_resource(TickBroadcastTimer::default());
        app.add_systems(
            FixedUpdate,
            send_tick_update.in_set(GameLogic::Start).after(tick),
        );
    }
}

fn send_tick_update(
    mut timer: ResMut<TickBroadcastTimer>,
    mut server: ResMut<RenetServer>,
    time: Res<Time>,
    tick: Res<Tick>,
) {
    timer.0.tick(time.delta());

    if timer.0.just_finished() {
        let message = ReliableMessageFromServer::TickSync(TickSync {
            tick: tick.get(),
            unix_millis: get_unix_millis(),
        });

        let bytes = bincode::encode_to_vec(message, bincode::config::standard()).unwrap();

        server.broadcast_message(DefaultChannel::ReliableUnordered, bytes);
    }
}
