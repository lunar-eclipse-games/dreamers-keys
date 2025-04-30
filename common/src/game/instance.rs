use rapier2d::na::Vector2;

type Vec2 = Vector2<f32>;

pub enum CollisionShape {
    Rectangle { min: Vec2, max: Vec2 },
    Wall { min: Vec2, max: Vec2 },
    Circle { center: Vec2, radius: f32 },
}
