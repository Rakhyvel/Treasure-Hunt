use std::path::Path;

use sdl2::{
    pixels::Color,
    rect::Rect,
    surface::Surface,
    ttf::{Font, Sdl2TtfContext},
};

use crate::App;

use super::{
    mesh::Mesh,
    objects::{create_program, Program, Texture, Uniform},
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
    program: Program,
}

impl Text {
    pub fn new(text: &'static str, font: Font, color: Color) -> Self {
        let mut surface = font.render(text).blended(color).unwrap();
        surface = surface
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .unwrap();
        let red_rect = Rect::new(100, 100, 200, 150);
        surface
            .fill_rect(Some(red_rect), Color::RGB(255, 255, 255))
            .unwrap();

        let texture = Texture::from_surface(surface);
        let mesh = Mesh::from_obj(QUAD_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0), texture);
        let program = create_program(
            include_str!("../shaders/2d.vert"),
            include_str!("../shaders/2d.frag"),
        )
        .unwrap();
        Self { mesh, program }
    }

    pub fn draw(&self, app: &App) {
        self.program.set();
        let u_resolution = Uniform::new(self.program.id(), "u_resolution").unwrap();
        let u_model_matrix = Uniform::new(self.program.id(), "u_model_matrix").unwrap();
        let mut model_matrix = nalgebra_glm::one();
        model_matrix = nalgebra_glm::translate(&model_matrix, &nalgebra_glm::vec3(0.0, 0.0, 0.0));
        model_matrix = nalgebra_glm::scale(&model_matrix, &nalgebra_glm::vec3(1.0, 1.0, 1.0));
        unsafe {
            gl::Uniform2f(
                u_resolution.id,
                app.screen_width as f32,
                app.screen_height as f32,
            );
            gl::UniformMatrix4fv(
                u_model_matrix.id,
                1,
                gl::FALSE,
                &model_matrix.columns(0, 4)[0],
            );
            self.mesh.set(self.program.id());
            gl::DrawElements(
                gl::TRIANGLES,
                self.mesh.indices_len(),
                gl::UNSIGNED_INT,
                0 as *const _,
            );
        }
    }
}
