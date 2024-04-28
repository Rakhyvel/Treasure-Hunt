use std::f32::consts::PI;

use nalgebra_glm::pi;
use rand::Rng;
use sdl2::keyboard::Scancode;

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
const SCALE: f32 = 2.0;
const UNIT_PER_METER: f32 = 0.1;
const PERSON_HEIGHT: f32 = 1.6764 * UNIT_PER_METER;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");

pub struct Island {
    world: World,

    tiles: Vec<f32>,
    grass_tile: Mesh,
    water_tiles: Mesh,
    program: Program,
    camera: Camera,
    vel_z: f32,
    feet_on_ground: bool,
    facing: f32,
    pitch: f32,

    t: f32,
    prev_jump: f32,
    sun_dir: nalgebra_glm::Vec3,
}

fn create_mesh(tiles: &Vec<f32>) -> (Vec<u16>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut indices = Vec::<u16>::new();
    let mut vertices = Vec::<f32>::new();
    let mut normals = Vec::<f32>::new();
    let mut uv = Vec::<f32>::new();
    let mut colors = Vec::<f32>::new();

    for y in 0..(MAP_SIZE - 1) {
        for x in 0..(MAP_SIZE - 1) {
            for i in 0..4 {
                // yx: 00 01 10 11
                let xo = i & 1;
                let yo = (i & 2) >> 1;
                let z = get_z(tiles, x + xo, y + yo);
                let z_scaled = get_z_scaled(tiles, x + xo, y + yo);

                add_vertex(&mut vertices, (x + xo) as f32, (y + yo) as f32, z_scaled);
                add_uv(&mut uv, xo as f32, yo as f32);
                if z > 0.75 {
                    colors.push(0.4);
                    colors.push(0.5);
                    colors.push(0.1);
                } else {
                    colors.push(0.9);
                    colors.push(0.9);
                    colors.push(0.7);
                }
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
                        get_z_scaled(tiles, x - 1, y) - get_z_scaled(tiles, x + 1, y),
                        get_z_scaled(tiles, x, y + 1) - get_z_scaled(tiles, x, y - 1),
                        0.2,
                    );
                    normal.normalize_mut();

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

    (indices, vertices, normals, uv, colors)
}

fn get_z(tiles: &Vec<f32>, x: usize, y: usize) -> f32 {
    tiles[x + y * MAP_SIZE]
}

fn get_z_scaled(tiles: &Vec<f32>, x: usize, y: usize) -> f32 {
    SCALE * get_z(tiles, x, y)
}

fn get_z_scaled_interpolated(tiles: &Vec<f32>, x: f32, y: f32) -> f32 {
    assert!(!x.is_nan());
    // The coordinates of the tile's origin (bottom left corner)
    let x_origin = x.floor();
    let y_origin = y.floor();

    // Coordinates inside the tile. [0,1]
    let x_offset = x - x_origin;
    let y_offset = y - y_origin;

    let ray_origin = nalgebra_glm::vec3(x, y, 10000.0);
    let ray_direction = nalgebra_glm::vec3(0.0, 0.0, -1.0);

    if y_offset <= 1.0 - x_offset {
        // In bottom triangle
        let (retval, _t) = intersect(
            nalgebra_glm::vec3(
                x_origin,
                y_origin,
                get_z_scaled(tiles, x_origin as usize, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z_scaled(tiles, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z_scaled(tiles, x_origin as usize, y_origin as usize + 1),
            ),
            ray_origin,
            ray_direction,
        )
        .unwrap();
        retval.z
    } else {
        // In top triangle
        let (retval, _t) = intersect(
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z_scaled(tiles, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin + 1.0,
                get_z_scaled(tiles, x_origin as usize + 1, y_origin as usize + 1),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z_scaled(tiles, x_origin as usize, y_origin as usize + 1),
            ),
            ray_origin,
            ray_direction,
        )
        .unwrap();
        retval.z
    }
}

fn intersect(
    v0: nalgebra_glm::Vec3,
    v1: nalgebra_glm::Vec3,
    v2: nalgebra_glm::Vec3,
    ray_origin: nalgebra_glm::Vec3,
    ray_direction: nalgebra_glm::Vec3,
) -> Option<(nalgebra_glm::Vec3, f32)> {
    const EPSILON: f32 = 0.0000001;
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = nalgebra_glm::cross(&ray_direction, &edge2);
    let a = nalgebra_glm::dot(&edge1, &h);

    if a.abs() < EPSILON {
        return None; // Ray is parallel to the triangle
    }

    let f = 1.0 / a;
    let s = ray_origin - v0;
    let u = f * nalgebra_glm::dot(&s, &h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = nalgebra_glm::cross(&s, &edge1);
    let v = f * nalgebra_glm::dot(&ray_direction, &q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * nalgebra_glm::dot(&edge2, &q);

    let intersection_point = ray_origin + t * ray_direction;
    Some((intersection_point, t))
}

fn add_vertex(vertices: &mut Vec<f32>, x: f32, y: f32, z: f32) {
    vertices.push(x);
    vertices.push(y);
    vertices.push(z);
}

fn add_uv(uv: &mut Vec<f32>, x: f32, y: f32) {
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
            let t = z * 0.7; // Tweak me to make the island smoother/perlinier
            let s: f32 = 0.25; // Tweak me to make the island pointier
            let m: f32 = 70.0; // Tweak me to make the island wider
            let bulge: f32 = (1.0 / (2.0 * pi::<f32>() * s.powf(2.0)))
                * (-((d / m).powf(2.0)) / (2.0 * s.powf(2.0))).exp();
            map[x + y * MAP_SIZE] = ((1.0 - t) * bulge + t * z).powf(1.0);
        }
    }
}

impl Scene for Island {
    fn update(&mut self, app: &App) {
        self.t += 1.0;

        self.control(app);

        self.vel_z -= 2.0 * UNIT_PER_METER / 62.5;
        self.camera.position.z += self.vel_z;
        let feet_height =
            get_z_scaled_interpolated(&self.tiles, self.camera.position.x, self.camera.position.y);
        println!(
            "Found it! ({}, {}, {})",
            self.camera.position.x, self.camera.position.y, feet_height
        );
        if self.camera.position.z - PERSON_HEIGHT <= feet_height {
            self.camera.position.z = feet_height + PERSON_HEIGHT;
            self.feet_on_ground = true;
            self.vel_z = 0.0;
        } else {
            self.feet_on_ground = false;
        }

        let rot_matrix = nalgebra_glm::rotate_y(
            &nalgebra_glm::rotate_z(&nalgebra_glm::one(), self.facing),
            self.pitch,
        );
        let facing_vec = (rot_matrix * nalgebra_glm::vec4(1.0, 0.0, 0.0, 0.0)).xyz();
        self.camera.lookat = self.camera.position + facing_vec;

        self.sun_dir = nalgebra_glm::vec3(0.0, 0.0, 1.0).normalize();
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
            let u_sun_dir = Uniform::new(self.program.id(), "u_sun_dir").unwrap();
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
            gl::Uniform3f(u_sun_dir.id, self.sun_dir.x, self.sun_dir.y, self.sun_dir.z);

            self.grass_tile.set(self.program.id());
            gl::DrawElements(
                gl::TRIANGLES,
                self.grass_tile.indices_len(),
                gl::UNSIGNED_INT,
                0 as *const _,
            );

            let mut model_matrix = nalgebra_glm::one();
            let pos = nalgebra_glm::vec3(50.0, 50.0, SCALE / 2.0);
            model_matrix = nalgebra_glm::translate(&model_matrix, &pos);
            model_matrix =
                nalgebra_glm::scale(&model_matrix, &nalgebra_glm::vec3(1000.0, 1000.0, 1.0));

            gl::UniformMatrix4fv(
                u_model_matrix.id,
                1,
                gl::FALSE,
                &model_matrix.columns(0, 4)[0],
            );

            self.water_tiles.set(self.program.id());
            gl::DrawElements(
                gl::TRIANGLES,
                self.water_tiles.indices_len(),
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
        let mut map = generate(MAP_SIZE, 10.5, rng.gen());
        create_bulge(&mut map);
        let mut spawn_point = nalgebra_glm::vec3((MAP_SIZE / 2) as f32, (MAP_SIZE / 2) as f32, 1.0);
        for x in (MAP_SIZE / 2)..MAP_SIZE {
            let height = get_z_scaled_interpolated(&map, x as f32, 50.0);
            if height < SCALE / 2.0 {
                spawn_point = nalgebra_glm::vec3(x as f32 - 1.0, 50.0, height + PERSON_HEIGHT);
                break;
            }
        }

        // let grass = Mesh::from_obj(QUAD_DATA, "res/earth.png");
        let (i, v, n, u, c) = create_mesh(&map);
        let grass = Mesh::new(i, vec![v, n, u, c], "res/grass.png");
        let water = Mesh::from_obj(QUAD_DATA, "res/water.png");
        let program = create_program().unwrap();
        program.set();
        let camera = Camera::new(
            spawn_point,
            nalgebra_glm::vec3(0.0, 0.0, 0.0),
            nalgebra_glm::vec3(0.0, 0.0, 1.0),
            0.94, // 50mm focal length (iPhone 13 camera)
        );

        Self {
            world,
            tiles: map,
            grass_tile: grass,
            water_tiles: water,
            program,
            camera,
            vel_z: 0.0,
            feet_on_ground: false,
            facing: 0.0,
            pitch: 0.0,
            t: 0.0,
            prev_jump: 0.0,
            sun_dir: nalgebra_glm::vec3(0.0, 0.0, 0.0),
        }
    }

    fn control(&mut self, app: &App) {
        let curr_w_state = app.keys[Scancode::W as usize];
        let curr_s_state = app.keys[Scancode::S as usize];
        let curr_a_state = app.keys[Scancode::A as usize];
        let curr_d_state = app.keys[Scancode::D as usize];
        let curr_space_state = app.keys[Scancode::Space as usize];
        const WALK_SPEED: f32 = 10.0 * UNIT_PER_METER / 62.5;
        let view_speed: f32 = 0.000005 * (app.screen_width as f32);
        let facing_vec = nalgebra_glm::vec3(self.facing.cos(), self.facing.sin(), 0.0);
        let sideways_vec = nalgebra_glm::cross(&self.camera.up, &facing_vec);
        let curr_height =
            get_z_scaled_interpolated(&self.tiles, self.camera.position.x, self.camera.position.y);
        if curr_w_state {
            let new_pos = self.camera.position + facing_vec * WALK_SPEED;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                self.camera.position = new_pos
            }
        }
        if curr_s_state {
            let new_pos = self.camera.position - facing_vec * WALK_SPEED;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                self.camera.position = new_pos
            }
        }
        if curr_a_state {
            let new_pos = self.camera.position + sideways_vec * WALK_SPEED;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                self.camera.position = new_pos
            }
        }
        if curr_d_state {
            let new_pos = self.camera.position - sideways_vec * WALK_SPEED;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                self.camera.position = new_pos
            }
        }
        if self.feet_on_ground {
            if curr_space_state {
                self.vel_z += 0.05;
            }
        }
        self.facing -= view_speed * app.mouse_rel_x as f32;
        self.pitch = (self.pitch + view_speed * (app.mouse_rel_y as f32))
            .max(view_speed - PI / 2.0)
            .min(PI / 2.0 - view_speed);
    }
}
