use std::marker::PhantomData;

use bevy::{
    app::{FixedUpdate, Plugin},
    ecs::schedule::{Condition, IntoSystemSetConfigs, SystemSet},
};

// pub mod game;
pub mod instance_message;
pub mod manager_message;
pub mod tick;
pub mod player;
pub mod net_obj;
pub mod physics;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameLogic {
    Start,
    ReadInput,
    /// Server: Send tick adjustments
    /// Client: Read tick adjustments
    TickAdjust,
    /// Server: spawn and despawn
    /// Client: receive spawns and despawns
    Spawn,
    /// Server: send data
    /// Client: receive data
    Sync,
    Game,
    End,
}

pub struct GameLogicPlugin<M, C: Condition<M> + Send + Sync> {
    condition: C,
    _phantom: PhantomData<M>,
}

impl<M, C: Condition<M> + Send + Sync> GameLogicPlugin<M, C> {
    pub fn new(condition: C) -> Self {
        Self {
            condition,
            _phantom: PhantomData,
        }
    }
}

impl<M: Send + Sync + 'static, C: Condition<M> + Send + Sync + 'static + Clone> Plugin
    for GameLogicPlugin<M, C>
{
    fn build(&self, app: &mut bevy::app::App) {
        let sets = (
            GameLogic::Start,
            GameLogic::TickAdjust,
            GameLogic::ReadInput,
            GameLogic::Spawn,
            GameLogic::Sync,
            GameLogic::Game,
            GameLogic::End,
        )
            .chain()
            .run_if(self.condition.clone());

        app.configure_sets(FixedUpdate, sets);
    }
}
