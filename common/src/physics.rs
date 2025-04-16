use bevy::{
    app::{App, FixedPostUpdate, Plugin},
    ecs::{
        entity::Entity,
        query::{QueryEntityError, QueryFilter},
    },
};
use bevy_rapier2d::{
    plugin::{
        NoUserData, RapierContext, RapierContextMut, RapierPhysicsPlugin, ReadRapierContext, WriteRapierContext
    },
    render::RapierDebugRenderPlugin,
};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_schedule(FixedPostUpdate));
        app.add_plugins(RapierDebugRenderPlugin::default());
    }
}

pub trait WriteRapierContextExt {
    fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError>;
    fn get_mut(&mut self, entity: Entity) -> Result<RapierContextMut, QueryEntityError>;
}

impl<T: QueryFilter + 'static> WriteRapierContextExt for WriteRapierContext<'_, '_, T> {
    fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError> {
        let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
            self.rapier_context.get(entity)?;

        Ok(RapierContext {
            simulation,
            colliders,
            joints,
            query_pipeline,
            rigidbody_set,
        })
    }

    fn get_mut(&mut self, entity: Entity) -> Result<RapierContextMut, QueryEntityError> {
        let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
            self.rapier_context.get_mut(entity)?;

        Ok(RapierContextMut {
            simulation,
            colliders,
            joints,
            query_pipeline,
            rigidbody_set,
        })
    }
}

pub trait ReadRapierContextExt {
    fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError>;
}

impl<T: QueryFilter + 'static> ReadRapierContextExt for ReadRapierContext<'_, '_, T> {
    fn get(&self, entity: Entity) -> Result<RapierContext, QueryEntityError> {
        let (simulation, colliders, joints, query_pipeline, rigidbody_set) =
            self.rapier_context.get(entity)?;

        Ok(RapierContext {
            simulation,
            colliders,
            joints,
            query_pipeline,
            rigidbody_set,
        })
    }
}
