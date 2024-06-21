use specs::prelude::*;
use std::path::Path;

use sdl2::{
    pixels::Color,
    ttf::{Font, Sdl2TtfContext},
};
use specs::{Component, DispatcherBuilder, VecStorage, World};

use crate::App;

use super::{
    camera::Camera,
    objects::{Program, Texture, Uniform},
    physics::PositionComponent,
    render3d::MeshMgrResource,
};

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

#[derive(Default)]
pub struct UIResource {
    pub camera: Camera,
    pub program: Program,
}

#[derive(Component)]
#[storage(VecStorage)]

pub struct QuadComponent {
    pub mesh_id: usize,
    pub width: i32,
    pub height: i32,
    pub opacity: f32,
    pub texture: Texture,
}

impl QuadComponent {
    pub fn from_texture(texture: Texture, width: i32, height: i32, quad_mesh_id: usize) -> Self {
        Self {
            mesh_id: quad_mesh_id,
            width,
            height,
            opacity: 1.0,
            texture,
        }
    }

    pub fn from_text(text: &'static str, font: &Font, color: Color, quad_mesh_id: usize) -> Self {
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
            width: width as i32,
            height: height as i32,
            opacity: 1.0,
            texture,
        }
    }
}

struct QuadSystem;
impl<'a> System<'a> for QuadSystem {
    type SystemData = (
        ReadStorage<'a, QuadComponent>,
        ReadStorage<'a, PositionComponent>,
        Read<'a, MeshMgrResource>,
        Read<'a, App>,
        Read<'a, UIResource>,
    );

    fn run(&mut self, (quads, positions, mesh_mgr, app, open_gl): Self::SystemData) {
        for (quad, position) in (&quads, &positions).join() {
            let mesh = mesh_mgr.data.get_mesh(quad.mesh_id);
            open_gl.program.set();
            quad.texture.activate(gl::TEXTURE0);
            quad.texture
                .associate_uniform(open_gl.program.id(), 0, "texture0");
            let u_opacity = Uniform::new(open_gl.program.id(), "u_opacity").unwrap();
            unsafe { gl::Uniform1f(u_opacity.id, quad.opacity) }
            mesh.draw(
                &open_gl.program,
                &open_gl.camera,
                position.pos,
                nalgebra_glm::vec3(
                    (quad.width as f32) / (app.screen_width as f32),
                    (quad.height as f32) / (app.screen_height as f32),
                    1.0,
                ),
            );
        }
    }
}

pub fn initialize_gui(world: &mut World, dispatcher_builder: &mut DispatcherBuilder) {
    // TODO: We will need an update and a render dispatch
    // Register GUI components
    world.register::<QuadComponent>();

    // Add GUI systems to the dispatcher
    dispatcher_builder.add(QuadSystem, "quad system", &[]);
}
