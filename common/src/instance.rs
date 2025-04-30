use hecs::{Entity, EntityBuilder, World};
use rapier2d::prelude::{ColliderBuilder, ColliderHandle, RigidBodyBuilder, RigidBodyHandle};
use std::{collections::HashMap, fmt::Debug, time::Duration};
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
    message::{OrderedInput, OwnedPlayerSync}, net_obj::{LastSyncTracker, NetworkObject}, physics::Physics, player::{apply_input, PlayerInput}, tick::Tick, Result, Vec2
};

pub struct Instance {
    id: Uuid,
    physics: Physics,
    world: World,
    tick: Tick,
}

#[derive(Debug)]
pub struct LocalPlayer;

#[derive(Debug)]
pub struct Player {}

#[derive(Debug)]
pub struct Position(pub Vec2);

#[derive(Debug, Default)]
pub struct LastInputTracker {
    pub order: u64,
}

impl LastInputTracker {
    pub fn new(order: u64) -> Self {
        LastInputTracker { order }
    }
}

impl Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Instance {
    pub fn new(id: Uuid) -> Instance {
        Instance {
            id,
            physics: Physics::new(),
            world: World::new(),
            tick: Tick::new(0),
        }
    }

    pub fn get_world(&self) -> &World {
        &self.world
    }

    pub fn get_world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn get_physics(&self) -> &Physics {
        &self.physics
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_tick(&self) -> Tick {
        self.tick
    }

    pub fn increment_tick(&mut self) {
        self.tick.increment();
    }

    pub fn set_tick(&mut self, tick: Tick) {
        self.tick = tick;
    }

    pub fn find_network_object(&self, needle: NetworkObject) -> Option<Entity> {
        for (entity, net_obj) in &mut self.world.query::<&NetworkObject>() {
            if needle == *net_obj {
                return Some(entity);
            }
        }

        None
    }

    pub fn spawn_player(
        &mut self,
        local_player: bool,
        position: Vec2,
        net_obj: NetworkObject,
        tick: Option<Tick>,
    ) -> Entity {
        let mut e = EntityBuilder::new();
        e.add(Player {})
            .add(Position(position))
            .add(net_obj)
            .add(LastInputTracker::default());

        let rb = self
            .physics
            .insert_rigid_body(RigidBodyBuilder::kinematic_position_based());

        let coll = self
            .physics
            .insert_collider_with_parent(ColliderBuilder::ball(50.0), rb);

        e.add(rb).add(coll);

        if local_player {
            e.add(LocalPlayer);
        }

        if let Some(tick) = tick {
            e.add(LastSyncTracker::<Position>::new(tick));
        }

        self.world.spawn(e.build())
    }

    pub fn despawn(&mut self, entity: Entity) {
        let rb = self.world.query_one_mut::<&RigidBodyHandle>(entity);

        match rb {
            Ok(handle) => self.physics.remove_rigid_body(*handle),
            Err(hecs::QueryOneError::Unsatisfied) => {}
            Err(hecs::QueryOneError::NoSuchEntity) => return,
        }

        self.world.despawn(entity).unwrap();
    }

    pub fn update_tick(&mut self) {
        self.tick.increment();
    }

    #[instrument]
    pub fn update(&mut self, dt: Duration) -> Result<()> {
        self.physics.update(&self.world);

        Ok(())
    }

    pub fn apply_inputs(&mut self, dt: f32, net_obj_inputs: &HashMap<NetworkObject, OrderedInput>) {
        for (_, (position, net_obj, last_input, collider, rigid_body, _)) in
            self.world.query_mut::<(
                &mut Position,
                &NetworkObject,
                &mut LastInputTracker,
                &ColliderHandle,
                &RigidBodyHandle,
                &mut Player,
            )>()
        {
            if let Some(input) = net_obj_inputs.get(net_obj) {
                apply_input(
                    &self.physics,
                    position,
                    &input.input,
                    *collider,
                    *rigid_body,
                    dt,
                );

                last_input.order = input.order;
            }
        }
    }

    pub fn check_and_rollback<F>(
        &mut self,
        player: Entity,
        owned_player_sync: &OwnedPlayerSync,
        dt: f32,
        inputs: Vec<OrderedInput>,
        mut save_snapshot: F,
    ) where
        F: FnMut(Vec2),
    {
        let Ok((position, collider, rigid_body)) =
            self.world
                .query_one_mut::<(&mut Position, &ColliderHandle, &RigidBodyHandle)>(player)
        else {
            return;
        };

        position.0 = Vec2::new(owned_player_sync.position[0], owned_player_sync.position[1]);

        for input in inputs {
            apply_input(
                &self.physics,
                position,
                &input.input,
                *collider,
                *rigid_body,
                dt,
            );

            save_snapshot(position.0);
        }
    }

    pub fn apply_input(&mut self, player: Entity, input: &PlayerInput, dt: f32) -> Option<Vec2> {
        let Ok((position, collider, rigid_body)) =
            self.world
                .query_one_mut::<(&mut Position, &ColliderHandle, &RigidBodyHandle)>(player)
        else {
            return None;
        };

        apply_input(
            &self.physics,
            position,
            input,
            *collider,
            *rigid_body,
            dt,
        );

        Some(position.0)
    }

    pub fn print_player_positions(&mut self) {
        for (_, position) in self.world.query_mut::<&Position>().with::<&Player>() {
            info!("{position:?}");
        }
    }
}
