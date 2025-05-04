use common::Vec2;
use nalgebra_glm as glm;

pub trait Camera {
    fn build_view_projection_matrix(&self) -> glm::Mat4;
}

#[derive(Debug)]
pub struct Camera2D {
    position: Vec2,
    size: Vec2,
}

impl Camera2D {
    pub fn new(position: Vec2, size: Vec2) -> Camera2D {
        Camera2D { position, size }
    }

    pub fn set_position(&mut self, new_position: Vec2) {
        self.position = new_position;
    }
}

impl Camera for Camera2D {
    fn build_view_projection_matrix(&self) -> glm::Mat4 {
        let min = self.position - self.size * 0.5;
        let max = self.position + self.size * 0.5;

        let proj = glm::ortho_zo(min.x, max.x, min.y, max.y, 0.0, 1.0);

        proj
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: glm::Mat4,
}

impl CameraUniform {
    pub fn new() -> CameraUniform {
        CameraUniform {
            view_proj: glm::identity(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &impl Camera) {
        self.view_proj = camera.build_view_projection_matrix();
    }
}
