use std::collections::HashMap;

use bevy::ecs::schedule::SystemSet;
use uuid::Uuid;

pub mod instance;
pub mod inventory;
pub mod item;

#[derive(Debug, Component, Clone)]
struct Account {
    id: Uuid,
    name: String,
}

#[derive(Debug)]
pub struct Game {
    /// move to database
    accounts: HashMap<Uuid, Account>,
    online_accounts: HashMap<Uuid, Entity>,
    world: World,
    authorization: bool,
}

impl Game {
    pub fn new() -> Self {
        Game {
            accounts: HashMap::new(),
            online_accounts: HashMap::new(),
            world: World::new(),
            authorization: true,
        }
    }

    pub fn local<S: Into<String>>(player_name: S) -> Self {
        let mut game = Game::new();

        let player_id = Uuid::now_v7();

        let account = Account {
            id: player_id,
            name: player_name.into(),
        };

        game.accounts.insert(player_id, account);

        game.authorization = false;

        game
    }
}