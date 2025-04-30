use std::fmt::Debug;

use hecs::World;
use rapier2d::{
    parry::query::{ShapeCastHit, ShapeCastOptions},
    prelude::*,
};

use crate::{Vec2, instance::Position};

pub struct Physics {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    query_pipeline: QueryPipeline,
    integration_parameters: IntegrationParameters,
}

impl Debug for Physics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Physics").finish_non_exhaustive()
    }
}

impl Default for Physics {
    fn default() -> Self {
        Physics::new()
    }
}

impl Physics {
    pub fn new() -> Physics {
        let rigid_body_set = RigidBodySet::new();
        let collider_set = ColliderSet::new();

        let integration_parameters = IntegrationParameters::default();
        let physics_pipeline = PhysicsPipeline::new();
        let island_manager = IslandManager::new();
        let broad_phase = DefaultBroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();
        let ccd_solver = CCDSolver::new();
        let query_pipeline = QueryPipeline::new();

        Physics {
            rigid_body_set,
            collider_set,
            physics_pipeline,
            island_manager,
            broad_phase,
            narrow_phase,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
            query_pipeline,
            integration_parameters,
        }
    }

    pub fn update(&mut self, world: &World) {
        for (_, (pos, rb)) in world.query::<(&Position, &RigidBodyHandle)>().iter() {
            self.rigid_body_set[*rb].set_position(pos.0.into(), false);
        }

        self.query_pipeline.update(&self.collider_set);
    }

    pub fn insert_rigid_body(&mut self, rigid_body: impl Into<RigidBody>) -> RigidBodyHandle {
        self.rigid_body_set.insert(rigid_body)
    }

    pub fn remove_rigid_body(&mut self, rigid_body: RigidBodyHandle) {
        self.rigid_body_set.remove(
            rigid_body,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true
        );
    }

    pub fn insert_collider(&mut self, collider: impl Into<Collider>) -> ColliderHandle {
        self.collider_set.insert(collider)
    }

    pub fn insert_collider_with_parent(
        &mut self,
        collider: impl Into<Collider>,
        rigid_body: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(collider, rigid_body, &mut self.rigid_body_set)
    }

    pub fn cast_shape(
        &self,
        shape_position: Vec2,
        shape_velocity: Vec2,
        shape: ColliderHandle,
        options: ShapeCastOptions,
        filter: QueryFilter<'_>,
    ) -> Option<(ColliderHandle, ShapeCastHit)> {
        self.query_pipeline.cast_shape(
            &self.rigid_body_set,
            &self.collider_set,
            &shape_position.into(),
            &shape_velocity,
            self.collider_set.get(shape).unwrap().shape(),
            options,
            filter,
        )
    }
}

// pub struct PhysicsPlugin;

// impl Plugin for PhysicsPlugin {
//     fn build(&self, app: &mut App) {
//         app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_schedule(FixedPostUpdate));
//         app.add_plugins(RapierDebugRenderPlugin::default());
//     }
// }

// pub trait WriteRapierContextExt {
//     fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError>;
//     fn get_mut(&mut self, entity: Entity) -> Result<RapierContextMut, QueryEntityError>;
// }

// impl<T: QueryFilter + 'static> WriteRapierContextExt for WriteRapierContext<'_, '_, T> {
//     fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError> {
//         let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
//             self.rapier_context.get(entity)?;

//         Ok(RapierContext {
//             simulation,
//             colliders,
//             joints,
//             query_pipeline,
//             rigidbody_set,
//         })
//     }

//     fn get_mut(&mut self, entity: Entity) -> Result<RapierContextMut, QueryEntityError> {
//         let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
//             self.rapier_context.get_mut(entity)?;

//         Ok(RapierContextMut {
//             simulation,
//             colliders,
//             joints,
//             query_pipeline,
//             rigidbody_set,
//         })
//     }
// }

// pub trait ReadRapierContextExt {
//     fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError>;
// }

// impl<T: QueryFilter + 'static> ReadRapierContextExt for ReadRapierContext<'_, '_, T> {
//     fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError> {
//         let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
//             self.rapier_context.get(entity)?;

//         Ok(RapierContext {
//             simulation,
//             colliders,
//             joints,
//             query_pipeline,
//             rigidbody_set,
//         })
//     }
// }
