use specs::prelude::*;
use std::path::Path;

use sdl2::{
    pixels::Color,
    ttf::{Font, Sdl2TtfContext},
};
use specs::{Component, DispatcherBuilder, VecStorage, World};

use crate::{scenes::island::UIResource, App};

use super::{mesh::MeshMgrResource, objects::Texture};

pub struct FontMgr {
    ttf_context: Sdl2TtfContext,
}

impl FontMgr {
    pub fn new() -> Self {
        let ttf_context = sdl2::ttf::init().unwrap();
        Self { ttf_context }
    }

    pub fn load_font(&self, path: &str, size: u16) -> Result<Font, String> {
        self.ttf_context
            .load_font(Path::new(path), size)
            .map_err(|e| e.to_string())
    }
}

pub struct Quad {
    mesh_id: usize,
    position: nalgebra_glm::Vec3,
    width: i32,
    height: i32,
    texture: Texture,
}

impl Quad {
    pub fn from_texture(
        texture: Texture,
        position: nalgebra_glm::Vec3,
        width: i32,
        height: i32,
        quad_mesh_id: usize,
    ) -> Self {
        Self {
            mesh_id: quad_mesh_id,
            position,
            width,
            height,
            texture,
        }
    }

    pub fn from_text(text: &'static str, font: Font, color: Color, quad_mesh_id: usize) -> Self {
        let surface = font
            .render(text)
            .blended(color)
            .unwrap()
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();

        let width = surface.width();
        let height = surface.height();

        let texture = Texture::from_surface(surface);
        Self {
            mesh_id: quad_mesh_id,
            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            width: width as i32,
            height: height as i32,
            texture,
        }
    }
}

impl Component for Quad {
    type Storage = VecStorage<Self>;
}

struct QuadSystem;
impl<'a> System<'a> for QuadSystem {
    type SystemData = (
        ReadStorage<'a, Quad>,
        Read<'a, MeshMgrResource>,
        Read<'a, App>,
        Read<'a, UIResource>,
    );

    fn run(&mut self, (text_components, mesh_mgr, app, open_gl): Self::SystemData) {
        for text in text_components.join() {
            let mesh = mesh_mgr.data.get_mesh(text.mesh_id);
            open_gl.program.set();
            text.texture.activate(gl::TEXTURE0);
            text.texture
                .associate_uniform(open_gl.program.id(), 0, "texture0");
            mesh.draw(
                &open_gl.program,
                &open_gl.camera,
                text.position,
                nalgebra_glm::vec3(
                    (text.width as f32) / (app.screen_width as f32),
                    (text.height as f32) / (app.screen_height as f32),
                    1.0,
                ),
            );
        }
    }
}

pub fn initialize_gui(world: &mut World, dispatcher_builder: &mut DispatcherBuilder) {
    // TODO: We will need an update and a render dispatch
    // Register GUI components
    world.register::<Quad>();

    // Add GUI systems to the dispatcher
    dispatcher_builder.add(QuadSystem, "quad system", &[]);
}
