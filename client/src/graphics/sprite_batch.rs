use common::{Rect, Vec2, Vec4};
use nalgebra_glm as glm;
use tracing::{error, trace};
use wgpu::util::DeviceExt;

use super::texture::{TextureId, TextureRegistry};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec2,
    pub colour: Vec4,
    pub texture: Vec2,
}

impl Vertex {
    pub const fn new(position: Vec2, colour: Vec4, texture: Vec2) -> Vertex {
        Vertex {
            position,
            colour,
            texture,
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 2]>() + mem::size_of::<[f32; 4]>())
                        as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[derive(Debug)]
struct SpriteBatchItem {
    texture: TextureId,
    tl: Vertex,
    tr: Vertex,
    bl: Vertex,
    br: Vertex,
    #[allow(unused)]
    layer: f32,
}

#[derive(Debug)]
pub struct SpriteBatch {
    batch_items: Vec<SpriteBatchItem>,
    vertices: Vec<Vertex>,
    vertex_buffer: wgpu::Buffer,
    vertex_buffer_size: u64,
    index_buffer: wgpu::Buffer,
}

const MAXIMUM_BATCH_SIZE: u16 = 256;

impl SpriteBatch {
    pub fn new(device: &wgpu::Device) -> SpriteBatch {
        let mut indices = Vec::with_capacity(MAXIMUM_BATCH_SIZE as usize * 6);

        for i in 0..MAXIMUM_BATCH_SIZE {
            indices.push(i * 4);
            indices.push(i * 4 + 3);
            indices.push(i * 4 + 1);
            indices.push(i * 4);
            indices.push(i * 4 + 2);
            indices.push(i * 4 + 3);
        }

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SpriteBatch Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        SpriteBatch {
            batch_items: Vec::new(),
            vertices: Vec::new(),
            vertex_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SpriteBatch Vertex Buffer"),
                size: 0,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            vertex_buffer_size: 0,
            index_buffer,
        }
    }

    pub fn draw(&mut self, texture_id: TextureId, position: Vec2) -> DrawCall {
        DrawCall {
            texture_id,
            position,
            source: None,
            colour: None,
            rotation: None,
            origin: None,
            scale: None,
        }
    }

    pub fn draw_detailed(
        &mut self,
        texture_registry: &TextureRegistry,
        texture_id: TextureId,
        position: Vec2,
        source: Option<Rect>,
        colour: Option<Vec4>,
        rotation: Option<f32>,
        origin: Option<Vec2>,
        scale: Option<Vec2>,
    ) {
        let Some(texture) = texture_registry.get(texture_id) else {
            error!("Texture {texture_id:?} not loaded!");
            return;
        };

        let source = source.unwrap_or_else(|| {
            Rect::new(
                glm::zero(),
                glm::vec2(texture.get_width_f32(), texture.get_height_f32()),
            )
        });

        let colour = colour.unwrap_or(Vec4::new(1.0, 1.0, 1.0, 1.0));

        let _rotation = rotation.unwrap_or(0.0); // TODO

        let origin = origin.unwrap_or_else(glm::zero);

        let scale = scale.unwrap_or_else(|| glm::vec2(1.0, 1.0));

        let scaled_origin = origin.component_mul(&scale);

        let bl = Vertex::new(
            position - scaled_origin,
            colour,
            Vec2::new(
                source.min.x / texture.get_width_f32(),
                source.max.y / texture.get_height_f32(),
            ),
        );

        let br = Vertex::new(
            position - scaled_origin + Vec2::new(source.width() * scale.x, 0.0),
            colour,
            Vec2::new(
                source.max.x / texture.get_width_f32(),
                source.max.y / texture.get_height_f32(),
            ),
        );

        let tl = Vertex::new(
            position - scaled_origin + Vec2::new(0.0, source.height() * scale.y),
            colour,
            Vec2::new(
                source.min.x / texture.get_width_f32(),
                source.min.y / texture.get_height_f32(),
            ),
        );

        let tr = Vertex::new(
            position - scaled_origin + Vec2::new(source.width() * scale.x, source.height() * scale.y),
            colour,
            Vec2::new(
                source.max.x / texture.get_width_f32(),
                source.min.y / texture.get_height_f32(),
            ),
        );

        let item = SpriteBatchItem {
            texture: texture_id,
            tl,
            tr,
            bl,
            br,
            layer: 0.0,
        };

        trace!("{item:?}");

        self.batch_items.push(item);
    }

    pub fn end(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_registry: &TextureRegistry,
        render_pass: &mut wgpu::RenderPass,
    ) {
        if self.batch_items.is_empty() {
            return;
        }

        let mut batches = Vec::new();

        let mut current_texture = self.batch_items[0].texture;
        let mut current_batch_start = 0;
        let item_count = self.batch_items.len() as u64;

        for (i, item) in self.batch_items.drain(..).enumerate() {
            if current_texture != item.texture {
                let current_batch_end = i as u64;

                while current_batch_end - current_batch_start > MAXIMUM_BATCH_SIZE as u64 {
                    batches.push((
                        current_texture,
                        current_batch_start,
                        current_batch_start + MAXIMUM_BATCH_SIZE as u64,
                    ));
                    current_batch_start += MAXIMUM_BATCH_SIZE as u64;
                }

                batches.push((current_texture, current_batch_start, current_batch_end));
                current_texture = item.texture;
                current_batch_start = i as u64;
            }

            self.vertices.reserve(4);
            self.vertices.push(item.tl);
            self.vertices.push(item.tr);
            self.vertices.push(item.bl);
            self.vertices.push(item.br);
        }

        batches.push((current_texture, current_batch_start, item_count));

        if (self.vertices.len() * std::mem::size_of::<Vertex>()) as u64 <= self.vertex_buffer_size {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        } else {
            let contents = bytemuck::cast_slice(&self.vertices);
            self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SpriteBatch Vertex Buffer"),
                contents,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            self.vertex_buffer_size = contents.len() as u64;
        }

        self.vertices.clear();

        for (texture, start, end) in batches {
            let Some(texture) = texture_registry.get(texture) else {
                error!("Texture {texture:?} not loaded!");
                continue;
            };

            render_pass.set_bind_group(0, texture.get_bind_group(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice((start * 4)..(end * 4)));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            render_pass.draw_indexed(0..((end - start) as u32 * 6), 0, 0..1);
        }
    }
}

pub struct DrawCall {
    texture_id: TextureId,
    position: Vec2,
    source: Option<Rect>,
    colour: Option<Vec4>,
    rotation: Option<f32>,
    origin: Option<Vec2>,
    scale: Option<Vec2>,
}

impl DrawCall {
    pub fn source(mut self, source: Rect) -> DrawCall {
        self.source = Some(source);
        self
    }

    pub fn colour(mut self, colour: Vec4) -> DrawCall {
        self.colour = Some(colour);
        self
    }

    pub fn rotation(mut self, rotation: f32) -> DrawCall {
        self.rotation = Some(rotation);
        self
    }

    pub fn origin(mut self, origin: Vec2) -> DrawCall {
        self.origin = Some(origin);
        self
    }

    pub fn scale(mut self, scale: Vec2) -> DrawCall {
        self.scale = Some(scale);
        self
    }

    pub fn scale_uniform(mut self, scale: f32) -> DrawCall {
        self.scale = Some(Vec2::new(scale, scale));
        self
    }

    pub fn draw(self, sprite_batch: &mut SpriteBatch, texture_registry: &TextureRegistry) {
        sprite_batch.draw_detailed(
            texture_registry,
            self.texture_id,
            self.position,
            self.source,
            self.colour,
            self.rotation,
            self.origin,
            self.scale,
        );
    }
}
