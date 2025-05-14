use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use common::{DT, Error, Result, Vec2, instance::Instance};
use glfw::PWindow;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    backend::BackendConnection, graphics::Graphics, input::KeyboardState, instance::InstanceData,
};

pub struct Game {
    graphics: Graphics,
    last_redraw: Instant,
    accumulator: Duration,
    backend: BackendConnection,
    instances: HashMap<Uuid, InstanceData>,
    got_ctrl_c: Arc<AtomicBool>,
    keyboard_state: KeyboardState,
}

impl std::fmt::Debug for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Game").finish_non_exhaustive()
    }
}

impl Game {
    pub fn new(
        backend: BackendConnection,
        window: Arc<PWindow>,
        instance_id: Uuid,
    ) -> Result<Game> {
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

        Ok(game)
    }

    fn get_current_player_position(&mut self) -> Option<Vec2> {
        let current_instance = self.backend.get_current_instance()?;
        let current_instance = self.instances.get_mut(&current_instance)?;
        current_instance.get_current_player_position()
    }

    #[tracing::instrument(skip(self))]
    #[profiling::function]
    fn update(&mut self, dt: Duration) -> Result<()> {
        self.backend.pre_update(dt)?;

        for instance in self.instances.values_mut() {
            instance.update(&mut self.backend, &self.keyboard_state, dt)?;
        }

        self.backend.post_update()?;

        self.keyboard_state.post_update();

        if let Some(position) = self.get_current_player_position() {
            self.graphics.post_update(position);
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    #[profiling::function]
    fn draw(&mut self) -> Result<()> {
        let player_position = self.get_current_player_position().unwrap_or_default();
        self.graphics.render(player_position)?;

        profiling::finish_frame!();

        Ok(())
    }

    #[tracing::instrument(skip(self, glfw, window, events))]
    pub fn run(
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
                            self.keyboard_state.press(key, mods);
                        }
                        glfw::Action::Release => {
                            self.keyboard_state.release(key, mods);
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            while self.accumulator >= DT {
                self.accumulator -= DT;

                self.update(DT)?;
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

    pub fn into_backend(self) -> BackendConnection {
        self.backend
    }
}
