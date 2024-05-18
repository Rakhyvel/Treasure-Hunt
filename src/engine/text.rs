use std::path::Path;

use sdl2::{
    pixels::Color,
    ttf::{Font, Sdl2TtfContext},
};

use crate::App;

use super::{
    camera::Camera,
    mesh::Mesh,
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

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");
pub struct Text {
    mesh: Mesh,
    width: i32,
    height: i32,
    program: Program,
}

impl Text {
    pub fn new(text: &'static str, font: Font, color: Color) -> Self {
        let surface = font
            .render(text)
            .blended(color)
            .unwrap()
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();

        let width = surface.width();
        let height = surface.height();

        let texture = Texture::from_surface(surface);
        let mesh = Mesh::from_obj(QUAD_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0), texture);
        let program = create_program(
            include_str!("../shaders/2d.vert"),
            include_str!("../shaders/2d.frag"),
        )
        .unwrap();
        Self {
            mesh,
            width: width as i32,
            height: height as i32,
            program,
        }
    }

    pub fn draw(&mut self, app: &App, camera: &Camera) {
        self.mesh.draw(
            &self.program,
            camera,
            nalgebra_glm::vec3(0.0, 0.0, 0.0),
            nalgebra_glm::vec3(
                (self.width as f32) / (app.screen_width as f32),
                (self.height as f32) / (app.screen_height as f32),
                1.0,
            ),
        );
    }
}
