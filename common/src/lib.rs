pub mod message;
pub mod net_obj;
pub mod physics;
pub mod player;
pub mod result;
pub mod tick;
pub mod game;
pub mod instance;

use std::time::Duration;

use rapier2d::na::Vector2;
pub use result::{Error, Result, ResultExt};

/// 60 FPS
pub const DT: Duration = Duration::from_nanos(16666666);

pub type Vec2 = Vector2<f32>;

pub use hecs::Entity;

// #[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub enum GameLogic {
//     Start,
//     ReadInput,
//     /// Server: Send tick adjustments
//     /// Client: Read tick adjustments
//     TickAdjust,
//     /// Server: spawn and despawn
//     /// Client: receive spawns and despawns
//     Spawn,
//     /// Server: send data
//     /// Client: receive data
//     Sync,
//     Game,
//     End,
// }

// pub struct GameLogicPlugin<M, C: Condition<M> + Send + Sync> {
//     condition: C,
//     _phantom: PhantomData<M>,
// }

// impl<M, C: Condition<M> + Send + Sync> GameLogicPlugin<M, C> {
//     pub fn new(condition: C) -> Self {
//         Self {
//             condition,
//             _phantom: PhantomData,
//         }
//     }
// }

// impl<M: Send + Sync + 'static, C: Condition<M> + Send + Sync + 'static + Clone> Plugin
//     for GameLogicPlugin<M, C>
// {
//     fn build(&self, app: &mut bevy::app::App) {
//         let sets = (
//             GameLogic::Start,
//             GameLogic::TickAdjust,
//             GameLogic::ReadInput,
//             GameLogic::Spawn,
//             GameLogic::Sync,
//             GameLogic::Game,
//             GameLogic::End,
//         )
//             .chain()
//             .run_if(self.condition.clone());

//         app.configure_sets(FixedUpdate, sets);
//     }
// }
