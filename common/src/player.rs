use bevy::{
    ecs::{component::Component, entity::Entity},
    math::{Vec2, Vec3, Vec3Swizzles},
    time::Time,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::*;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Component)]
#[require(RigidBody(player_rigid_body), Collider(player_collider))]
pub struct Player {}

impl Player {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Player {
    fn default() -> Self {
        Player::new()
    }
}

fn player_rigid_body() -> RigidBody {
    RigidBody::KinematicPositionBased
}

fn player_collider() -> Collider {
    Collider::ball(50.0)
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerInput {
    pub move_direction: [f32; 2],
}

pub fn apply_input(
    context: &RapierContext,
    transform: &mut Transform,
    input: &PlayerInput,
    shape: &Collider,
    time: &Time,
    curr_player: Entity,
) {
    let speed = 500.0;
    let movement = Vec2::from(input.move_direction).normalize_or_zero() * speed * time.delta_secs();

    let out = move_character(
        context,
        movement,
        shape,
        transform.translation.xy(),
        QueryFilter::default().exclude_collider(curr_player),
    );

    transform.translation += Vec3::new(out.x, out.y, 0.0);
}

fn move_character(
    context: &RapierContext,
    movement: Vec2,
    shape: &Collider,
    shape_translation: Vec2,
    mut filter: QueryFilter,
) -> Vec2 {
    let mut translation_remaining = movement;

    let mut effective_translation = Vec2::ZERO;

    let offset = 2.0;
    let mut iters_remaining = 5;

    while translation_remaining.length_squared() > 0.0 && iters_remaining > 0 {
        if let Some((hit_entity, hit)) = context.cast_shape(
            shape_translation + effective_translation,
            0.0,
            translation_remaining,
            shape,
            ShapeCastOptions {
                target_distance: offset,
                stop_at_penetration: false,
                max_time_of_impact: 1.0,
                compute_impact_geometry_on_penetration: true,
            },
            filter,
        ) {
            let hit_details = hit.details.unwrap();

            // We hit something, compute and apply the allowed interference-free translation.
            let allowed_dist = hit.time_of_impact;
            let allowed_translation = movement * allowed_dist;
            effective_translation += allowed_translation;
            translation_remaining -= allowed_translation;

            // Slide along hit normal plane projection
            translation_remaining = project_on_plane(translation_remaining, hit_details.normal1)
                .normalize()
                * translation_remaining.length();
            filter = filter.exclude_collider(hit_entity);
        } else {
            // No interference along the path.
            effective_translation += translation_remaining;
            break;
        }

        iters_remaining -= 1;
    }

    effective_translation
}

fn project_on_plane(dir: Vec2, plane_normal: Vec2) -> Vec2 {
    let sqr_len = plane_normal.length_squared();

    let dot = dir.dot(plane_normal);

    Vec2::new(
        dir.x - plane_normal.x * dot / sqr_len,
        dir.y - plane_normal.y * dot / sqr_len,
    )
}
