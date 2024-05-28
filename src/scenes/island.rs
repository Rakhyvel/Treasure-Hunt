use std::{
    f32::consts::PI,
    sync::{Arc, Mutex},
};

use nalgebra_glm::pi;
use rand::Rng;
use sdl2::{keyboard::Scancode, pixels::Color};
use specs::{prelude::*, Component, Join, ReadStorage};

use crate::{
    engine::{
        camera::{Camera, ProjectionKind},
        mesh::{Mesh, MeshMgr, MeshMgrResource},
        objects::{create_program, Program, Texture},
        perlin::*,
        text::{initialize_gui, FontMgr, Text},
    },
    App, Scene,
};

const MAP_SIZE: usize = 512;
const SCALE: f32 = 10.0;
const UNIT_PER_METER: f32 = 0.2;
const PERSON_HEIGHT: f32 = 1.6764 * UNIT_PER_METER;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");
pub const CONE_DATA: &[u8] = include_bytes!("../../res/cone.obj");
pub const CUBE_DATA: &[u8] = include_bytes!("../../res/cube.obj");

/*
 * RESOURCES
 */
#[derive(Default)]
struct TickResource {
    t: f32,
}

#[derive(Default)]
struct OpenGlResource {
    camera: Camera,
    program: Program,
}

#[derive(Default)]
struct TileResource {
    tiles: Vec<f32>,
}

#[derive(Default)]
struct PlayerResource {
    vel: nalgebra_glm::Vec3,
    feet_on_ground: bool,
    facing: f32,
    pitch: f32,
}

/*
 * COMPONENTS
 */
#[derive(Component)]
struct Renderable {
    mesh_id: usize,
    position: nalgebra_glm::Vec3,
    scale: nalgebra_glm::Vec3,
    texture: Texture,
    render_dist: Option<f32>, //< When Some, only render when the position is this close to the camera
}

/*
 * SYSTEMS
 */
struct SkySystem;
impl<'a> System<'a> for SkySystem {
    type SystemData = (
        Read<'a, App>,
        Read<'a, OpenGlResource>,
        Read<'a, TickResource>,
    );
    fn run(&mut self, (app, open_gl, tick_res): Self::SystemData) {
        let model_t = tick_res.t * 0.0001 + 0.4;
        unsafe {
            let day_color = nalgebra_glm::vec3(172.0, 205.0, 248.0);
            let night_color = nalgebra_glm::vec3(5.0, 6.0, 7.0);
            let red_color = nalgebra_glm::vec3(124.0, 102.0, 86.0);
            let do_color = if model_t.cos() > 0.0 {
                day_color
            } else {
                night_color
            };
            let dnf = model_t.sin().powf(10.0);
            let result = dnf * red_color + (1.0 - dnf) * do_color;
            gl::ClearColor(result.x / 255., result.y / 255., result.z / 255., 1.0);
        }

        Mesh::set_3d(
            &open_gl.program,
            nalgebra_glm::vec3(0.0, model_t.sin(), model_t.cos()),
            nalgebra_glm::vec2(app.screen_width as f32, app.screen_height as f32),
        );
    }
}

struct RenderSystem;
impl<'a> System<'a> for RenderSystem {
    type SystemData = (
        ReadStorage<'a, Renderable>,
        Read<'a, MeshMgrResource>,
        Read<'a, OpenGlResource>,
    );

    fn run(&mut self, (render_comps, mesh_mgr, open_gl): Self::SystemData) {
        for renderable in render_comps.join() {
            match renderable.render_dist {
                Some(d) => {
                    if nalgebra_glm::length(&(renderable.position - open_gl.camera.position)) > d {
                        continue;
                    }
                }
                None => {}
            }
            let mesh = mesh_mgr.data.get_mesh(renderable.mesh_id);
            open_gl.program.set();
            renderable
                .texture
                .activate(gl::TEXTURE0, open_gl.program.id());
            mesh.draw(
                &open_gl.program,
                &open_gl.camera,
                renderable.position,
                renderable.scale,
            );
        }
    }
}

struct PlayerSystem;
impl<'a> System<'a> for PlayerSystem {
    type SystemData = (
        Write<'a, PlayerResource>,
        Read<'a, App>,
        Write<'a, OpenGlResource>,
        Read<'a, TileResource>,
    );

    fn run(&mut self, (mut player, app, mut opengl, tile_res): Self::SystemData) {
        // TODO: This is a lot. Can it be cleaned up somehow?
        let curr_w_state = app.keys[Scancode::W as usize];
        let curr_s_state = app.keys[Scancode::S as usize];
        let curr_a_state = app.keys[Scancode::A as usize];
        let curr_d_state = app.keys[Scancode::D as usize];
        let curr_space_state = app.keys[Scancode::Space as usize];
        let curr_shift_state = app.keys[Scancode::LShift as usize];
        let walking = curr_w_state || curr_s_state || curr_a_state || curr_d_state;
        let walk_speed: f32 = 1.0 * PERSON_HEIGHT / 62.5 * if curr_shift_state { 1.5 } else { 1.0 };
        let view_speed: f32 = 0.000005 * (app.screen_width as f32);
        let facing_vec = nalgebra_glm::vec3(player.facing.cos(), player.facing.sin(), 0.0);
        let sideways_vec = nalgebra_glm::cross(&opengl.camera.up, &facing_vec);
        let mut player_vel_vec = nalgebra_glm::vec3(0.0, 0.0, 0.0);
        if curr_w_state {
            player_vel_vec += facing_vec;
        }
        if curr_s_state {
            player_vel_vec += -facing_vec;
        }
        if curr_a_state {
            player_vel_vec += sideways_vec;
        }
        if curr_d_state {
            player_vel_vec += -sideways_vec;
        }
        if curr_space_state && player.feet_on_ground {
            player.vel.z += 0.2 * UNIT_PER_METER;
        } else if walking {
            player.vel += player_vel_vec.normalize() * walk_speed; // Move the player, this way moving diagonal isn't faster
        }
        player.facing -= view_speed * app.mouse_rel_x as f32;
        player.pitch = (player.pitch + view_speed * (app.mouse_rel_y as f32))
            .max(view_speed - PI / 2.0)
            .min(PI / 2.0 - view_speed);

        player.vel.z -= 0.01 * UNIT_PER_METER; // gravity
        opengl.camera.position += player.vel; // integration position with velocity
        let feet_height = get_z_scaled_interpolated(
            &tile_res.tiles,
            opengl.camera.position.x,
            opengl.camera.position.y,
        );
        if opengl.camera.position.z - PERSON_HEIGHT <= feet_height {
            let normal = get_normal(
                &tile_res.tiles,
                opengl.camera.position.x,
                opengl.camera.position.y,
            );
            let d = feet_height - (opengl.camera.position.z - PERSON_HEIGHT);
            player.vel += normal * 0.1 * d; // normal from slopes
            if !walking {
                let feet_normal = -nalgebra_glm::vec3(normal.x, normal.y, 0.0);
                player.vel += feet_normal * 0.1 * d; // if standing still, remove the side-to-side component from the slope normal, so there's no slipping
            }
            // If the player is a meter deep into the earth, hard bump them
            let bump_limit = UNIT_PER_METER * 0.01;
            if feet_height - opengl.camera.position.z >= bump_limit {
                opengl.camera.position.z = feet_height - bump_limit;
            }

            player.feet_on_ground = true;
            player.vel *= 0.8; // friction
        } else {
            player.feet_on_ground = false;
            player.vel.x *= 0.8;
            player.vel.y *= 0.8;
        }

        let rot_matrix = nalgebra_glm::rotate_y(
            &nalgebra_glm::rotate_z(&nalgebra_glm::one(), player.facing),
            player.pitch,
        );
        let facing_vec = (rot_matrix * nalgebra_glm::vec4(1.0, 0.0, 0.0, 0.0)).xyz();
        opengl.camera.lookat = opengl.camera.position + facing_vec;
    }
}

struct TickSystem;
impl<'a> System<'a> for TickSystem {
    type SystemData = Write<'a, TickResource>;

    fn run(&mut self, mut tick_res: Self::SystemData) {
        tick_res.t += 1.0;
    }
}

/*
 * SCENE STUFF
 */
pub struct Island {
    world: World,
    update_dispatcher: Dispatcher<'static, 'static>,
    render_dispatcher: Dispatcher<'static, 'static>,
    _ui_camera: Arc<Mutex<Camera>>, // TODO: Probably remove too
}

impl Scene for Island {
    fn update(&mut self, app: &App) {
        self.world.insert((*app).clone());
        self.update_dispatcher.dispatch_seq(&mut self.world);
    }

    fn render(&mut self, _app: &App) {
        self.render_dispatcher.dispatch_seq(&mut self.world);
    }
}

impl Island {
    pub fn new() -> Self {
        // Setup ECS the world
        let mut world = World::new();
        world.register::<Renderable>();

        // Setup the dispatchers
        let mut update_dispatcher_builder = DispatcherBuilder::new();
        update_dispatcher_builder.add(PlayerSystem, "player system", &[]);
        update_dispatcher_builder.add(TickSystem, "tick system", &[]);

        let mut render_dispatcher_builder = DispatcherBuilder::new();
        render_dispatcher_builder.add(SkySystem, "sky system", &[]);
        render_dispatcher_builder.add(RenderSystem, "render system", &[]);
        initialize_gui(&mut world, &mut render_dispatcher_builder);

        // Setup island map
        let mut rng = rand::thread_rng();
        let mut map = generate(MAP_SIZE, 0.1, rng.gen());
        create_bulge(&mut map);
        erosion(&mut map, MAP_SIZE, 51.0);
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

        // Setup the font manager
        let font_mgr = FontMgr::new();
        let font = font_mgr
            .load_font("res/HelveticaNeue Medium.ttf", 24)
            .unwrap();

        // Setup the mesh manager
        let mut mesh_mgr = MeshMgr::new();
        let (i, v, n, u, c) = create_mesh(&map);
        let grass_mesh = mesh_mgr.add_mesh(Mesh::new(i, vec![v, n, u, c]));
        let quad_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(QUAD_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let cube_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(CUBE_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let tree_mesh = mesh_mgr.add_mesh(Mesh::from_obj(
            CONE_DATA,
            nalgebra_glm::vec3(0.2, 0.25, 0.0),
        ));
        world.insert(MeshMgrResource { data: mesh_mgr });

        // Setup the program and cameras
        let ui_camera = Arc::new(Mutex::new(Camera::new(
            nalgebra_glm::vec3(0.0, 0.0, 1.0),
            nalgebra_glm::vec3(0.0, 0.0, 0.0),
            nalgebra_glm::vec3(0.0, 1.0, 0.0),
            ProjectionKind::Orthographic,
        )));

        // Add entities
        world
            .create_entity()
            .with(Renderable {
                mesh_id: grass_mesh,
                position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                texture: Texture::from_png("res/grass.png"),
                render_dist: None,
            })
            .build();
        world
            .create_entity()
            .with(Renderable {
                mesh_id: quad_mesh,
                position: nalgebra_glm::vec3(0.0, 0.0, SCALE * 0.5),
                scale: nalgebra_glm::vec3(1000.0, 1000.0, 1000.0),
                texture: Texture::from_png("res/water.png"),
                render_dist: None,
            })
            .build();
        world
            .create_entity()
            .with(Text::new(
                "+",
                font,
                Color::RGBA(255, 255, 255, 255),
                Arc::clone(&ui_camera),
                quad_mesh,
            ))
            .build();
        for _ in 0..(MAP_SIZE * 2) {
            // Add all the trees
            let mut attempts = 0;
            loop {
                let (x, y) = (
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                );
                let height = get_z_scaled_interpolated(&map, x, y);
                let dot_prod = get_dot_prod(&map, x, y).abs();
                if height >= SCALE && dot_prod > 0.75 {
                    world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: tree_mesh,
                            position: nalgebra_glm::vec3(x, y, height),
                            scale: nalgebra_glm::vec3(1.5, 1.5, 3.0),
                            texture: Texture::from_png("res/grass.png"),
                            render_dist: Some(128.0),
                        })
                        .build();
                    break;
                }
                if attempts > 100 {
                    break;
                }
                attempts += 1;
            }
        }
        for _ in 0..(MAP_SIZE / 85) {
            // Add all the treasure boxes
            let mut attempts = 0;
            loop {
                let (x, y) = (
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                );
                let height = get_z_scaled_interpolated(&map, x, y);
                let dot_prod = get_dot_prod(&map, x, y).abs();
                if height >= 0.5 * SCALE
                    && height <= 0.8 * SCALE
                    && height / SCALE < 0.75 * dot_prod
                {
                    world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: cube_mesh,
                            position: nalgebra_glm::vec3(x, y, height),
                            scale: nalgebra_glm::vec3(0.1, 0.1, 3.1),
                            texture: Texture::from_png("res/grass.png"),
                            render_dist: Some(128.0),
                        })
                        .build();
                    break;
                }
                if attempts > 100 {
                    break;
                }
                attempts += 1;
            }
        }

        // Add resources
        world.insert(App::default());
        world.insert(TickResource { t: 0.0 });
        world.insert(OpenGlResource {
            camera: Camera::new(
                spawn_point,
                nalgebra_glm::vec3(MAP_SIZE as f32 / 2.0, MAP_SIZE as f32 / 2.0, SCALE / 2.0),
                nalgebra_glm::vec3(0.0, 0.0, 1.0),
                ProjectionKind::Perspective { fov: 0.9 },
            ),
            program: create_program(include_str!("../.vert"), include_str!("../.frag")).unwrap(),
        });
        world.insert(PlayerResource {
            vel: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            feet_on_ground: true,
            facing: 3.14,
            pitch: 0.0,
        });
        world.insert(TileResource { tiles: map });

        Self {
            world,
            _ui_camera: ui_camera,
            update_dispatcher: update_dispatcher_builder.build(),
            render_dispatcher: render_dispatcher_builder.build(),
        }
    }
}

fn create_bulge(map: &mut Vec<f32>) {
    for y in 0..MAP_SIZE {
        for x in 0..MAP_SIZE {
            let z = map[x + y * MAP_SIZE];
            let xo = (x as f32) - (MAP_SIZE as f32) / 2.0;
            let yo = (y as f32) - (MAP_SIZE as f32) / 2.0;
            let d = ((xo * xo + yo * yo) as f32).sqrt();
            let t = 0.6; // Tweak me to make the island smoother/perlinier
            let s: f32 = z * 0.1 + 0.15 - 0.2 * (d / MAP_SIZE as f32); // Tweak me to make the island pointier
            let m: f32 = MAP_SIZE as f32 * 0.7; // Tweak me to make the island wider
            let bulge: f32 = (1.0 / (2.0 * pi::<f32>() * s.powf(2.0)))
                * (-((d / m).powf(2.0)) / (2.0 * s.powf(2.0))).exp();
            map[x + y * MAP_SIZE] = (1.0 - t) * bulge + t * z;
        }
    }
}

fn create_mesh(tiles: &Vec<f32>) -> (Vec<u32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut indices = Vec::<u32>::new();
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
    indices: &mut Vec<u32>,
    vertices: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    uv: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    x: f32,
    y: f32,
    offsets: &Vec<(f32, f32)>,
    i: &mut u32,
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

    let edge1 = tri_verts[1] - tri_verts[0];
    let edge2 = tri_verts[2] - tri_verts[0];
    let normal = nalgebra_glm::cross(&edge1, &edge2).normalize();
    for _ in 0..3 {
        normals.push(normal.x);
        normals.push(normal.y);
        normals.push(normal.z);
    }
    // 0 = steep
    // 1 = flat
    let dot_prod = nalgebra_glm::dot(&normal, &nalgebra_glm::vec3(0.0, 0.0, 1.0));

    let avg_z = sum_z / 3.0;
    for _ in 0..3 {
        if avg_z < 0.75 * dot_prod {
            // sand
            colors.push(0.8);
            colors.push(0.7);
            colors.push(0.6);
        } else if avg_z * 0.5 > dot_prod || dot_prod < 0.75 {
            // stone
            colors.push(0.5);
            colors.push(0.45);
            colors.push(0.4);
        } else {
            // grass
            colors.push(0.3);
            colors.push(0.4);
            colors.push(0.2);
        }
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

fn get_normal(tiles: &Vec<f32>, x: f32, y: f32) -> nalgebra_glm::Vec3 {
    assert!(!x.is_nan());
    // The coordinates of the tile's origin (bottom left corner)
    let x_origin = x.floor();
    let y_origin = y.floor();

    // Coordinates inside the tile. [0,1]
    let x_offset = x - x_origin;
    let y_offset = y - y_origin;

    if y_offset <= 1.0 - x_offset {
        // In bottom triangle
        tri_normal(
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
        )
    } else {
        // In top triangle
        tri_normal(
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
        )
    }
}

fn get_dot_prod(tiles: &Vec<f32>, x: f32, y: f32) -> f32 {
    assert!(!x.is_nan());

    nalgebra_glm::dot(&get_normal(tiles, x, y), &nalgebra_glm::vec3(0.0, 0.0, 1.0))
}

fn tri_normal(
    v0: nalgebra_glm::Vec3,
    v1: nalgebra_glm::Vec3,
    v2: nalgebra_glm::Vec3,
) -> nalgebra_glm::Vec3 {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let normal = nalgebra_glm::cross(&edge1, &edge2).normalize();
    normal
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
