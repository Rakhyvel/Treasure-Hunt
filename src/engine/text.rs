use specs::prelude::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use sdl2::{
    pixels::Color,
    ttf::{Font, Sdl2TtfContext},
};
use specs::{Component, DispatcherBuilder, VecStorage, World};

use crate::App;

use super::{
    camera::Camera,
    mesh::MeshMgrResource,
    objects::{create_program, Program, Texture},
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

#[derive(Component)]
#[storage(VecStorage)]
pub struct Text {
    mesh_id: usize,
    position: nalgebra_glm::Vec3,
    width: i32,
    height: i32,
    texture: Texture,
    camera: Arc<Mutex<Camera>>,
    program: Arc<Mutex<Program>>,
}
struct TextSystem;

impl<'a> System<'a> for TextSystem {
    type SystemData = (
        ReadStorage<'a, Text>,
        Read<'a, MeshMgrResource>,
        Read<'a, App>,
    );

    fn run(&mut self, (text_components, mesh_mgr, app): Self::SystemData) {
        for text in text_components.join() {
            let program_guard = text.program.as_ref().try_lock().unwrap();
            let camera_guard = text.camera.as_ref().try_lock().unwrap();
            let mesh = mesh_mgr.data.get_mesh(text.mesh_id);
            program_guard.set();
            text.texture.activate(gl::TEXTURE0, program_guard.id());
            mesh.draw(
                &program_guard,
                &camera_guard,
                text.position,
                nalgebra_glm::vec3(
                    (text.width as f32) / (app.screen_width as f32),
                    (text.height as f32) / (app.screen_height as f32),
                    1.0,
                ),
            );
            drop(camera_guard);
            drop(program_guard);
        }
    }
}

impl Text {
    pub fn new(
        text: &'static str,
        font: Font,
        color: Color,
        camera: Arc<Mutex<Camera>>,
        quad_mesh_id: usize,
    ) -> Self {
        let surface = font
            .render(text)
            .blended(color)
            .unwrap()
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();

        let width = surface.width();
        let height = surface.height();

        let texture = Texture::from_surface(surface);
        let program = Arc::new(Mutex::new(
            create_program(
                include_str!("../shaders/2d.vert"),
                include_str!("../shaders/2d.frag"),
            )
            .unwrap(),
        ));
        Self {
            mesh_id: quad_mesh_id,
            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            width: width as i32,
            height: height as i32,
            texture,
            camera,
            program: Arc::clone(&program),
        }
    }
}

pub fn initialize_gui(world: &mut World, dispatcher_builder: &mut DispatcherBuilder) {
    // TODO: We will need an update and a render dispatch
    // Register GUI components
    world.register::<Text>();

    // Add GUI systems to the dispatcher
    dispatcher_builder.add(TextSystem, "text_system", &[]);
}
