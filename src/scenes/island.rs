use std::{
    f32::consts::PI,
    sync::{Arc, Mutex},
};

use nalgebra_glm::pi;
use rand::Rng;
use sdl2::{keyboard::Scancode, pixels::Color, render};
use specs::{prelude::*, Component, Join, ReadStorage};

use crate::{
    engine::{
        camera::{Camera, ProjectionKind},
        mesh::{self, Mesh, MeshMgr},
        objects::{create_program, Program, Texture},
        perlin::*,
        text::{FontMgr, Text},
    },
    App, Scene,
};

const MAP_SIZE: usize = 100;
const SCALE: f32 = 2.0;
const UNIT_PER_METER: f32 = 0.2;
const PERSON_HEIGHT: f32 = 1.6764 * UNIT_PER_METER;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");
pub const CONE_DATA: &[u8] = include_bytes!("../../res/cone.obj");

#[derive(Component)]
struct Renderable {
    mesh_id: usize,
    position: nalgebra_glm::Vec3,
    scale: nalgebra_glm::Vec3,
    program: Arc<Mutex<Program>>, // TODO: These should probably be resources, too, maybe
    camera: Arc<Mutex<Camera>>,   // TODO: These should probably be resources, too, maybe
}

#[derive(Default)]
struct MeshMgrResource {
    pub data: MeshMgr,
}

struct RenderSystem;

impl<'a> System<'a> for RenderSystem {
    type SystemData = (ReadStorage<'a, Renderable>, Read<'a, MeshMgrResource>);

    fn run(&mut self, (render_comps, mesh_mgr): Self::SystemData) {
        for renderable in render_comps.join() {
            let program_guard = renderable.program.as_ref().try_lock().unwrap();
            let camera_guard = renderable.camera.as_ref().try_lock().unwrap();
            let mesh = mesh_mgr.data.get_mesh(renderable.mesh_id);
            mesh.draw(
                &program_guard,
                &camera_guard,
                renderable.position,
                renderable.scale,
            );
            drop(camera_guard);
            drop(program_guard);
        }
    }
}

pub struct Island {
    world: World,

    tiles: Vec<f32>,

    // text: Text,
    program: Arc<Mutex<Program>>,
    camera: Arc<Mutex<Camera>>,
    // ui_camera: Arc<Mutex<Camera>>,
    vel_z: f32,
    feet_on_ground: bool,
    facing: f32,
    pitch: f32,

    t: f32,
}

fn create_mesh(tiles: &Vec<f32>) -> (Vec<u16>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut indices = Vec::<u16>::new();
    let mut vertices = Vec::<f32>::new();
    let mut normals = Vec::<f32>::new();
    let mut uv = Vec::<f32>::new();
    let mut colors = Vec::<f32>::new();

    let mut i = 0;
    for y in 0..(MAP_SIZE - 1) {
        for x in 0..(MAP_SIZE - 1) {
            // Left triangle |\
            let offsets = vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
            add_triangle(
                tiles,
                &mut indices,
                &mut vertices,
                &mut normals,
                &mut uv,
                &mut colors,
                x as f32,
                y as f32,
                &offsets,
                &mut i,
            );

            // Right triangle \|
            let offsets = vec![(1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
            add_triangle(
                tiles,
                &mut indices,
                &mut vertices,
                &mut normals,
                &mut uv,
                &mut colors,
                x as f32,
                y as f32,
                &offsets,
                &mut i,
            );
        }
    }

    (indices, vertices, normals, uv, colors)
}

fn add_triangle(
    tiles: &Vec<f32>,
    indices: &mut Vec<u16>,
    vertices: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    uv: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    x: f32,
    y: f32,
    offsets: &Vec<(f32, f32)>,
    i: &mut u16,
) {
    let mut sum_z = 0.0;
    let tri_verts: Vec<nalgebra_glm::Vec3> = offsets
        .iter()
        .map(|(xo, yo)| {
            let z_scaled = get_z_scaled(tiles, (x + xo) as usize, (y + yo) as usize);
            let mapval = nalgebra_glm::vec3(x + xo, y + yo, z_scaled);
            sum_z += get_z(tiles, (x + xo) as usize, (y + yo) as usize);
            add_vertex(vertices, x + xo, y + yo, z_scaled);
            add_uv(uv, *xo as f32, *yo as f32);
            indices.push(*i);
            *i += 1;
            mapval
        })
        .collect();

    let avg_z = sum_z / 3.0;
    for _ in 0..3 {
        if avg_z > 0.75 {
            colors.push(0.4);
            colors.push(0.5);
            colors.push(0.1);
        } else {
            colors.push(0.9);
            colors.push(0.9);
            colors.push(0.7);
        }
    }

    let edge1 = tri_verts[1] - tri_verts[0];
    let edge2 = tri_verts[2] - tri_verts[0];
    let normal = nalgebra_glm::cross(&edge1, &edge2).normalize();
    for _ in 0..3 {
        normals.push(normal.x);
        normals.push(normal.y);
        normals.push(normal.z);
    }
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
            let t = 0.6; // Tweak me to make the island smoother/perlinier
            let s: f32 = 0.25; // Tweak me to make the island pointier
            let m: f32 = MAP_SIZE as f32 * 0.7; // Tweak me to make the island wider
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

        let mut camera_guard = self.camera.as_ref().lock().unwrap();

        self.vel_z -= 1.3 * UNIT_PER_METER / 62.5;
        camera_guard.position.z += self.vel_z;
        let feet_height = get_z_scaled_interpolated(
            &self.tiles,
            camera_guard.position.x,
            camera_guard.position.y,
        );
        if camera_guard.position.z - PERSON_HEIGHT <= feet_height {
            camera_guard.position.z = feet_height + PERSON_HEIGHT;
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
        camera_guard.lookat = camera_guard.position + facing_vec;

        drop(camera_guard);
    }

    fn render(&mut self, app: &App) {
        unsafe {
            let day_color = nalgebra_glm::vec3(172.0, 205.0, 248.0);
            let night_color = nalgebra_glm::vec3(5.0, 6.0, 7.0);
            let red_color = nalgebra_glm::vec3(124.0, 102.0, 86.0);
            let do_color = if (self.t * 0.001).cos() > 0.0 {
                day_color
            } else {
                night_color
            };
            let dnf = (self.t * 0.001).sin().powf(10.0);
            let result = dnf * red_color + (1.0 - dnf) * do_color;
            gl::ClearColor(result.x / 255., result.y / 255., result.z / 255., 1.0);
        }

        let program_guard = self.program.as_ref().lock().unwrap();
        Mesh::set_3d(
            &program_guard,
            nalgebra_glm::vec3(0.0, (self.t * 0.001).sin(), (self.t * 0.001).cos()),
            nalgebra_glm::vec2(app.screen_width as f32, app.screen_height as f32),
        );
        drop(program_guard);

        let mut dispatcher = DispatcherBuilder::new()
            .with(RenderSystem, "movement_system", &[])
            .build();
        dispatcher.dispatch_seq(&mut self.world);
    }
}

impl Island {
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Renderable>();

        let mut rng = rand::thread_rng();
        let mut map = generate(MAP_SIZE, 0.1, rng.gen());
        create_bulge(&mut map);
        let mut spawn_point = nalgebra_glm::vec3((MAP_SIZE / 2) as f32, (MAP_SIZE / 2) as f32, 1.0);
        for x in (MAP_SIZE / 2)..MAP_SIZE {
            let height = get_z_scaled_interpolated(&map, x as f32, MAP_SIZE as f32 / 2.0);
            if height < SCALE / 2.0 {
                spawn_point = nalgebra_glm::vec3(
                    x as f32 - 1.0,
                    MAP_SIZE as f32 / 2.0,
                    height + PERSON_HEIGHT,
                );
                break;
            }
        }

        // TODO: Add text back!
        // let font_mgr = FontMgr::new();
        // let font = font_mgr
        //     .load_font("res/HelveticaNeue Medium.ttf", 24)
        //     .unwrap();
        // let text = Text::new("+", font, Color::RGBA(255, 255, 255, 255));

        // TODO: Add trees!
        // let tree = Mesh::from_obj(
        //     CONE_DATA,
        //     nalgebra_glm::vec3(0.2, 0.25, 0.0),
        //     Texture::from_png("res/grass.png"),
        // );

        let program = Arc::new(Mutex::new(
            create_program(include_str!("../.vert"), include_str!("../.frag")).unwrap(),
        ));
        let camera = Arc::new(Mutex::new(Camera::new(
            spawn_point,
            nalgebra_glm::vec3(0.0, 0.0, 0.0),
            nalgebra_glm::vec3(0.0, 0.0, 1.0),
            ProjectionKind::Perspective { fov: 0.9 },
        )));
        // TODO: Add text back!
        // let ui_camera = Arc::new(Mutex::new(Camera::new(
        //     nalgebra_glm::vec3(0.0, 0.0, 1.0),
        //     nalgebra_glm::vec3(0.0, 0.0, 0.0),
        //     nalgebra_glm::vec3(0.0, 1.0, 0.0),
        //     ProjectionKind::Orthographic,
        // )));

        let (i, v, n, u, c) = create_mesh(&map);
        let mut mesh_mgr = MeshMgr::new();
        let grass_mesh = mesh_mgr.add_mesh(Mesh::new(
            i,
            vec![v, n, u, c],
            Texture::from_png("res/grass.png"),
        ));
        let water_mesh = mesh_mgr.add_mesh(Mesh::from_obj(
            QUAD_DATA,
            nalgebra_glm::vec3(1.0, 1.0, 1.0),
            Texture::from_png("res/water.png"),
        ));
        let tree_mesh = mesh_mgr.add_mesh(Mesh::from_obj(
            CONE_DATA,
            nalgebra_glm::vec3(0.2, 0.25, 0.0),
            Texture::from_png("res/grass.png"),
        ));

        world.insert(MeshMgrResource { data: mesh_mgr });
        world
            .create_entity()
            .with(Renderable {
                mesh_id: grass_mesh,
                position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                program: Arc::clone(&program),
                camera: Arc::clone(&camera),
            })
            .build();
        world
            .create_entity()
            .with(Renderable {
                mesh_id: water_mesh,
                position: nalgebra_glm::vec3(0.0, 0.0, SCALE * 0.5),
                scale: nalgebra_glm::vec3(1000.0, 1000.0, 1000.0),
                program: Arc::clone(&program),
                camera: Arc::clone(&camera),
            })
            .build();

        for _ in 0..MAP_SIZE {
            loop {
                let (x, y) = (
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                );
                let height = get_z_scaled_interpolated(&map, x, y);
                if height >= SCALE {
                    world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: tree_mesh,
                            position: nalgebra_glm::vec3(x, y, height),
                            scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                            program: Arc::clone(&program),
                            camera: Arc::clone(&camera),
                        })
                        .build();
                    break;
                }
            }
        }

        Self {
            world,
            tiles: map,
            // text,
            // trees: tree_pos,
            program,
            camera,
            // ui_camera,
            vel_z: 0.0,
            feet_on_ground: false,
            facing: 0.0,
            pitch: 0.0,
            t: 0.0,
        }
    }

    fn control(&mut self, app: &App) {
        let mut camera_gaurd = self.camera.as_ref().lock().unwrap();

        let curr_w_state = app.keys[Scancode::W as usize];
        let curr_s_state = app.keys[Scancode::S as usize];
        let curr_a_state = app.keys[Scancode::A as usize];
        let curr_d_state = app.keys[Scancode::D as usize];
        let curr_shift_state = app.keys[Scancode::LShift as usize];
        let curr_space_state = app.keys[Scancode::Space as usize];
        let walk_speed: f32 =
            10.0 * UNIT_PER_METER / 62.5 * if curr_shift_state { 2.0 } else { 1.0 };
        let view_speed: f32 = 0.000005 * (app.screen_width as f32);
        let facing_vec = nalgebra_glm::vec3(self.facing.cos(), self.facing.sin(), 0.0);
        let sideways_vec = nalgebra_glm::cross(&camera_gaurd.up, &facing_vec);
        let curr_height = get_z_scaled_interpolated(
            &self.tiles,
            camera_gaurd.position.x,
            camera_gaurd.position.y,
        );
        if curr_w_state {
            let new_pos = camera_gaurd.position + facing_vec * walk_speed;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                camera_gaurd.position = new_pos
            }
        }
        if curr_s_state {
            let new_pos = camera_gaurd.position - facing_vec * walk_speed;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                camera_gaurd.position = new_pos
            }
        }
        if curr_a_state {
            let new_pos = camera_gaurd.position + sideways_vec * walk_speed;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                camera_gaurd.position = new_pos
            }
        }
        if curr_d_state {
            let new_pos = camera_gaurd.position - sideways_vec * walk_speed;
            let new_height = get_z_scaled_interpolated(&self.tiles, new_pos.x, new_pos.y);
            if !self.feet_on_ground || curr_height <= SCALE / 2.0 || new_height > SCALE / 2.0 {
                camera_gaurd.position = new_pos
            }
        }
        if self.feet_on_ground {
            if curr_space_state {
                self.vel_z += 0.5 * UNIT_PER_METER;
            }
        }
        self.facing -= view_speed * app.mouse_rel_x as f32;
        self.pitch = (self.pitch + view_speed * (app.mouse_rel_y as f32))
            .max(view_speed - PI / 2.0)
            .min(PI / 2.0 - view_speed);

        drop(camera_gaurd);
    }
}
