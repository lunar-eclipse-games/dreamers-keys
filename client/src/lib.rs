use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use backend::BackendConnection;
use common::{DT, Error, Result, game::character::CharacterKind, instance::Instance};
use graphics::Graphics;
use instance::InstanceData;
use tracing::{Level, info, instrument, span, warn};
use uuid::Uuid;
// use winit::{
//     application::ApplicationHandler,
//     dpi::LogicalSize,
//     event::WindowEvent,
//     event_loop::{ControlFlow, EventLoop},
//     window::Window,
// };
// use sdl2::video::Window;
use glfw::PWindow;

pub mod backend;
pub mod graphics;
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

    let mut game = Game {
        graphics: pollster::block_on(Graphics::new(window.clone()))?,
        last_redraw: Instant::now(),
        accumulator: Duration::ZERO,
        backend,
        instances: HashMap::new(),
        got_ctrl_c: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        keyboard_state: KeyboardState::default(),
    };
    game.instances
        .insert(instance_id, InstanceData::new(Instance::new(instance_id)));

    ctrlc::set_handler({
        let got_ctrl_c = game.got_ctrl_c.clone();
        move || got_ctrl_c.store(true, Ordering::SeqCst)
    })
    .unwrap();

    game.run(glfw, window, events)?;

    game.backend.shutdown()?;

    Ok(())
}

pub struct Game {
    graphics: Graphics,
    last_redraw: Instant,
    accumulator: Duration,
    backend: BackendConnection,
    instances: HashMap<Uuid, InstanceData>,
    got_ctrl_c: Arc<AtomicBool>,
    keyboard_state: KeyboardState,
}

impl Debug for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Game").finish_non_exhaustive()
    }
}

impl Game {
    #[instrument]
    #[inline(never)]
    fn update(&mut self, dt: Duration) -> Result<()> {
        self.backend.pre_update(dt)?;

        for instance in self.instances.values_mut() {
            instance.update(&mut self.backend, &self.keyboard_state, dt)?;
        }

        self.backend.post_update()?;

        Ok(())
    }

    #[instrument]
    fn draw(&mut self) -> Result<()> {
        self.graphics.render()?;

        Ok(())
    }

    #[instrument(skip(self, glfw, window, events))]
    fn run(
        &mut self,
        mut glfw: glfw::Glfw,
        window: Arc<PWindow>,
        events: glfw::GlfwReceiver<(f64, glfw::WindowEvent)>,
    ) -> Result<()> {
        while !window.should_close() {
            let elapsed = self.last_redraw.elapsed();
            self.accumulator += elapsed;
            self.last_redraw = Instant::now();

            glfw.poll_events();
            for (_, event) in glfw::flush_messages(&events) {
                match event {
                    glfw::WindowEvent::FramebufferSize(w, h) => {
                        self.graphics.resize(Some((w, h)));
                    }
                    glfw::WindowEvent::Key(key, _, action, mods) => match action {
                        glfw::Action::Press => {
                            self.keyboard_state.pressed.insert(key, mods);
                            self.keyboard_state.just_pressed.insert(key, mods);
                        }
                        glfw::Action::Release => {
                            self.keyboard_state.just_released.insert(key, mods);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            while self.accumulator >= DT {
                self.accumulator -= DT;

                self.update(DT)?;

                self.keyboard_state.just_pressed.clear();
                self.keyboard_state.clear_released();
                self.keyboard_state.just_released.clear();
            }

            if self.got_ctrl_c.load(Ordering::SeqCst) {
                info!("Got CTRL-C. Exiting...");
                break;
            }

            if let Err(err) = self.draw() {
                match err {
                    Error::Surface(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        self.graphics.resize(None);
                    }
                    Error::Surface(wgpu::SurfaceError::Timeout) => {
                        warn!("Surface timeout");
                    }
                    Error::Surface(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other)
                    | _ => {
                        return Err(err);
                    }
                }
            }

            std::thread::sleep(DT.saturating_sub(self.last_redraw.elapsed()));
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct KeyboardState {
    pressed: HashMap<glfw::Key, glfw::Modifiers>,
    just_pressed: HashMap<glfw::Key, glfw::Modifiers>,
    just_released: HashMap<glfw::Key, glfw::Modifiers>,
}

impl KeyboardState {
    pub fn is_pressed(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(pressed) = self.pressed.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *pressed == mods
        } else {
            true
        }
    }

    pub fn is_just_pressed(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(just_pressed) = self.just_pressed.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *just_pressed == mods
        } else {
            true
        }
    }

    pub fn is_just_released(&self, key: glfw::Key, mods: Option<glfw::Modifiers>) -> bool {
        let Some(just_released) = self.just_released.get(&key) else {
            return false;
        };

        if let Some(mods) = mods {
            *just_released == mods
        } else {
            true
        }
    }

    fn clear_released(&mut self) {
        self.pressed
            .retain(|k, _| !self.just_released.contains_key(k));
    }
}
