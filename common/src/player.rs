use bincode::{Decode, Encode};
use rapier2d::{
    parry::query::ShapeCastOptions,
    prelude::{ColliderHandle, QueryFilter, RigidBodyHandle},
};
use serde::{Deserialize, Serialize};

use crate::{Vec2, instance::Position, physics::Physics};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerInput {
    pub move_direction: [f32; 2],
}

#[profiling::function]
pub fn apply_input(
    physics: &Physics,
    position: &mut Position,
    input: &PlayerInput,
    shape: ColliderHandle,
    curr_player: RigidBodyHandle,
    dt: f32,
) {
    let speed = 500.0;
    let movement = if input.move_direction == [0.0, 0.0] {
        Vec2::zeros()
    } else {
        Vec2::from(input.move_direction).normalize() * speed * dt
    };

    let out = move_character(
        physics,
        movement,
        shape,
        position.0,
        QueryFilter::default().exclude_rigid_body(curr_player),
    );

    position.0 += out;
}

#[profiling::function]
fn move_character(
    physics: &Physics,
    movement: Vec2,
    shape: ColliderHandle,
    shape_translation: Vec2,
    mut filter: QueryFilter,
) -> Vec2 {
    let mut translation_remaining = movement;

    let mut effective_translation = Vec2::zeros();

    let offset = 2.0;
    let mut iters_remaining = 5;

    while translation_remaining.norm_squared() > 1.0e-6 && iters_remaining > 0 {
        if let Some((hit_entity, hit)) = physics.cast_shape(
            shape_translation + effective_translation,
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
            // We hit something, compute and apply the allowed interference-free translation.
            let allowed_dist = hit.time_of_impact;
            let allowed_translation = movement * allowed_dist;
            effective_translation += allowed_translation;
            translation_remaining -= allowed_translation;

            // Slide along hit normal plane projection
            let projection = project_on_plane(translation_remaining, &hit.normal1);
            if projection.norm_squared() > 1.0e-6 {
                translation_remaining = projection.normalize() * translation_remaining.norm();
            } else {
                translation_remaining = Vec2::zeros();
            }

            // filter = filter.exclude_collider(hit_entity);
        } else {
            // No interference along the path.
            effective_translation += translation_remaining;
            break;
        }

        iters_remaining -= 1;
    }

    effective_translation
}

fn project_on_plane(dir: Vec2, plane_normal: &Vec2) -> Vec2 {
    let sqr_len = plane_normal.norm_squared();

    let dot = dir.dot(plane_normal);

    Vec2::new(
        dir.x - plane_normal.x * dot / sqr_len,
        dir.y - plane_normal.y * dot / sqr_len,
    )
}
