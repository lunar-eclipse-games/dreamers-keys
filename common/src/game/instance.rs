use bevy_ecs::{component::Component, entity::Entity, system::Commands};
use glam::Vec2;
use uuid::Uuid;

#[derive(Component)]
pub struct Instance {
    pub owner: Uuid,
    pub home_of: Option<Uuid>,
    pub collision_shapes: Vec<CollisionShape>,
}

impl Instance {
    pub fn is_home(&self) -> bool {
        self.home_of.is_some()
    }
}

pub enum CollisionShape {
    Rectangle { min: Vec2, max: Vec2 },
    Wall { min: Vec2, max: Vec2 },
    Circle { center: Vec2, radius: f32 },
}

pub fn create_home(commands: &mut Commands, owner: Uuid) -> Entity {
    commands
        .spawn(Instance {
            owner,
            home_of: Some(owner),
            collision_shapes: vec![CollisionShape::Circle {
                center: Vec2::ZERO,
                radius: 100.0,
            }],
        })
        .id()
}
