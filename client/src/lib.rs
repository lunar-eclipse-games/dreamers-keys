use std::sync::Arc;

use backend::BackendConnection;
use common::{Result, game::character::CharacterKind};
use game::Game;
use input::KeyboardState;
use tracing::{Level, info, span};

pub mod backend;
pub mod game;
pub mod graphics;
pub mod input;
pub mod instance;

pub fn run() -> Result<()> {
    let span = span!(Level::INFO, "client");
    let _enter = span.enter();

    let mut backend = BackendConnection::local();

    let character = backend.create_character("testington", CharacterKind::SoloAccount)?;

    let instance_id = backend.enter_game(character.character_id)?;

    info!("Entered game with character \"{}\"", character.name);

    let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

    glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));

    let (window, events) = glfw.with_primary_monitor(|glfw, monitor| {
        let (mut window, events) = glfw
            .create_window(1920, 1080, "Dreamer's Keys", glfw::WindowMode::Windowed)
            .unwrap();

        window.set_key_polling(true);
        window.set_framebuffer_size_polling(true);

        if let Some(monitor) = monitor {
            let (mx, my, mw, mh) = monitor.get_workarea();
            let (w, h) = window.get_size();
            window.set_pos(mx + mw / 2 - w / 2, my + mh / 2 - h / 2);
        }

        (Arc::new(window), events)
    });

    let mut game = Game::new(backend, window.clone(), instance_id)?;

    game.run(glfw, window, events)?;

    game.into_backend().shutdown()?;

    Ok(())
}
