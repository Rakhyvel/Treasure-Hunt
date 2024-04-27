use rand::Rng;
use sdl2::{render::Canvas, video::Window};

use crate::{
    engine::{
        camera::Camera,
        mesh::Mesh,
        objects::{create_program, Program, Uniform},
        perlin::*,
        world::World,
    },
    App, Scene,
};

const MAP_SIZE: usize = 100;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");

pub struct Island {
    world: World,

    tiles: Vec<f32>,
    grass_tile: Mesh,
    program: Program,
    camera: Camera,
    i: f32,
}

fn create_mesh(tiles: &Vec<f32>) -> (Vec<u16>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut indices = Vec::<u16>::new();
    let mut vertices = Vec::<f32>::new();
    let mut normals = Vec::<f32>::new();
    let mut uv = Vec::<f32>::new();

    for y in 0..(MAP_SIZE - 1) {
        for x in 0..(MAP_SIZE - 1) {
            for i in 0..4 {
                let yo = (i & 2) >> 1;
                let xo = i & 1;
                let z = get_z(tiles, x + xo, y + yo);

                add_vertex(&mut vertices, (x + xo) as f32, (y + yo) as f32, z);
                add_uv(&mut uv, xo as f32, yo as f32, 0.0);
            }
        }
    }

    for y in 0..(MAP_SIZE - 1) {
        for x in 0..(MAP_SIZE - 1) {
            for i in 0..4 {
                let xo = i & 1;
                let yo = (i & 2) >> 1;
                let x = x + xo;
                let y = y + yo;
                if x == 0 || y == 0 || x >= MAP_SIZE - 2 || y >= MAP_SIZE - 2 {
                    normals.push(0.0);
                    normals.push(0.0);
                    normals.push(1.0);
                } else {
                    let mut normal = nalgebra_glm::vec3(
                        get_z(tiles, x - 1, y) - get_z(tiles, x + 1, y),
                        get_z(tiles, x, y + 1) - get_z(tiles, x, y - 1),
                        0.0,
                    );
                    // normal.normalize_mut();

                    normals.push(normal.x);
                    normals.push(normal.y);
                    normals.push(1.0);
                }
            }
        }
    }

    for i in 0..(MAP_SIZE * MAP_SIZE) {
        indices.push(4 * i as u16 + 0);
        indices.push(4 * i as u16 + 1);
        indices.push(4 * i as u16 + 2);

        indices.push(4 * i as u16 + 1);
        indices.push(4 * i as u16 + 3);
        indices.push(4 * i as u16 + 2);
    }

    (indices, vertices, normals, uv)
}

fn get_z(tiles: &Vec<f32>, x: usize, y: usize) -> f32 {
    10.0 * tiles[x + y * MAP_SIZE]
}

fn add_vertex(vertices: &mut Vec<f32>, x: f32, y: f32, z: f32) {
    vertices.push(x);
    vertices.push(y);
    vertices.push(z);
}

fn add_uv(uv: &mut Vec<f32>, x: f32, y: f32, z: f32) {
    uv.push(x);
    uv.push(y);
    uv.push(0.0);
}

fn create_bulge(map: &mut Vec<f32>) {
    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let z = map[x + y * MAP_SIZE];
            let xo = (x as f32) - (MAP_SIZE as f32) / 2.0;
            let yo = (y as f32) - (MAP_SIZE as f32) / 2.0;
            let d = ((xo * xo + yo * yo) as f32).sqrt();
            let t = 0.4; // modulation factor
                         // we basically have a "bulge" carrier, which is then averaged with the perlin noise so that there's an island
                         // in the center.
            map[x + y * MAP_SIZE] = (1.0 - t) * (-0.005 * (d * d)).exp() + t * z;
        }
    }
}

impl Scene for Island {
    fn update(&mut self, _app: &App) {
        self.i += 0.01;
        self.camera.position.x = self.i.cos() * 50.0 + 50.0;
        self.camera.position.y = self.i.sin() * 50.0 + 50.0;
        self.camera.position.z = 10.0;
    }

    fn render(&mut self, app: &App) {
        self.program.set();
        let (x, y) = (0.0, 0.0);
        let pos = nalgebra_glm::vec3((x as f32) * 1.0, (y as f32) * 1.0, 0.0);
        let mut model_matrix = nalgebra_glm::one();
        model_matrix = nalgebra_glm::translate(&model_matrix, &pos);
        let (view_matrix, proj_matrix) = self.camera.gen_view_proj_matrices();

        unsafe {
            // These Uniforms allow us to pass data (ex: window size, elapsed time) to the GPU shaders
            let u_model_matrix = Uniform::new(self.program.id(), "u_model_matrix").unwrap();
            let u_view_matrix = Uniform::new(self.program.id(), "u_view_matrix").unwrap();
            let u_proj_matrix = Uniform::new(self.program.id(), "u_proj_matrix").unwrap();
            let u_resolution = Uniform::new(self.program.id(), "u_resolution").unwrap();
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
            gl::UniformMatrix4fv(
                u_view_matrix.id,
                1,
                gl::FALSE,
                &view_matrix.columns(0, 4)[0],
            );
            gl::UniformMatrix4fv(
                u_proj_matrix.id,
                1,
                gl::FALSE,
                &proj_matrix.columns(0, 4)[0],
            );

            self.grass_tile.set(self.program.id());

            gl::DrawElements(
                gl::TRIANGLES,
                self.grass_tile.indices_len(),
                gl::UNSIGNED_INT,
                0 as *const _,
            );
        }
    }
}

impl Island {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let world = World::new();
        let mut map = generate(MAP_SIZE, 0.1, rng.gen());
        create_bulge(&mut map);

        // let grass = Mesh::from_obj(QUAD_DATA, "res/earth.png");
        let (i, v, n, u) = create_mesh(&map);
        let grass = Mesh::new(i, v, n, u, "res/grass.png");
        let program = create_program().unwrap();
        program.set();
        let camera = Camera::new(
            nalgebra_glm::vec3(0.0, 0.0, 5.0),
            nalgebra_glm::vec3(50.0, 50.0, 0.0),
            nalgebra_glm::vec3(0.0, 0.0, 1.0),
            0.94, // 50mm focal length (iPhone 13 camera)
        );

        Self {
            world,
            tiles: map,
            grass_tile: grass,
            program,
            camera,
            i: 0.0,
        }
    }
}
