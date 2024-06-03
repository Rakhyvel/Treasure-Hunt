use std::f32::consts::PI;

use nalgebra_glm::pi;
use rand::Rng;
use sdl2::{keyboard::Scancode, pixels::Color};
use specs::{prelude::*, Component, Join, ReadStorage};

use crate::{
    engine::{
        aabb::AABB,
        camera::{Camera, ProjectionKind},
        frustrum::Frustrum,
        mesh::{Mesh, MeshMgr, MeshMgrResource},
        objects::{create_program, Fbo, Program, Texture, Uniform},
        perlin::*,
        text::{initialize_gui, FontMgr, Quad},
    },
    App, Scene,
};

const MAP_SIZE: usize = 256;
const SCALE: f32 = MAP_SIZE as f32 / 128.0;
const UNIT_PER_METER: f32 = 0.1;
const PERSON_HEIGHT: f32 = 1.6764 * UNIT_PER_METER;
const SHADOW_SIZE: i32 = 2048;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");
pub const CONE_DATA: &[u8] = include_bytes!("../../res/cone.obj");
pub const CUBE_DATA: &[u8] = include_bytes!("../../res/cube.obj");
pub const MOB_DATA: &[u8] = include_bytes!("../../res/mob.obj");

/*
 * RESOURCES
 */
#[derive(Default)]
struct TickResource {
    t: f32,
}

#[derive(Default)]
pub struct OpenGlResource {
    // TODO: Put in engine I think
    pub camera: Camera,
    pub program: Program,
}

#[derive(Default)]
pub struct UIResource {
    // TODO: Put in engine I think
    pub camera: Camera,
    pub program: Program,
}

#[derive(Default)]
struct SunResource {
    shadow_camera: Camera,
    shadow_program: Program,
    fbo: Fbo,
    depth_map: Texture,
    light_dir: nalgebra_glm::Vec3,
}

#[derive(Default)]
struct TileResource {
    tiles: Vec<f32>,
}

/*
 * COMPONENTS
 */
#[derive(Component)]
#[storage(HashMapStorage)]
struct Player {
    feet_on_ground: bool,
    facing: f32,
    pitch: f32,
    prev_mouse_left_down: bool,
}

#[derive(Component)]
struct Renderable {
    mesh_id: usize,
    scale: nalgebra_glm::Vec3,
    texture: Texture,
    render_dist: Option<f32>, //< When Some, only render when the position is this close to the camera
}

#[derive(Component)]
struct Position {
    pos: nalgebra_glm::Vec3,
}

#[derive(Component)]
struct Velocity {
    vel: nalgebra_glm::Vec3,
}

#[derive(Default)]
struct CastsShadow;
impl Component for CastsShadow {
    type Storage = NullStorage<Self>;
}

#[derive(Component)]
#[storage(VecStorage)]
struct TreasureMap {
    treasure_entity: Entity,
    found: bool,
}

#[derive(Component)]
#[storage(VecStorage)]
struct Mob {}

#[derive(Component)]
#[storage(VecStorage)]
struct Projectile {}

#[derive(Component)]
#[storage(VecStorage)]
struct Collidable {
    aabb: AABB,
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
        Write<'a, SunResource>,
    );
    fn run(&mut self, (app, open_gl, tick_res, mut sun): Self::SystemData) {
        const MIN_PER_DAY: f32 = 60.0;
        // Noon:     0.0
        // Evening:  1.57
        // Midnight: 3.14
        // Morning:  4.71
        // Noon2:    6.28
        let model_t = tick_res.t / (MIN_PER_DAY * 60.0 * 62.6) + 5.3;
        unsafe {
            let day_color = nalgebra_glm::vec3(172.0, 205.0, 248.0);
            let night_color = nalgebra_glm::vec3(5.0, 6.0, 7.0);
            let red_color = nalgebra_glm::vec3(124.0, 102.0, 86.0);
            let do_color = if model_t.cos() > 0.0 {
                day_color
            } else {
                night_color
            };
            let dnf = model_t.sin().powf(100.0);
            let result = dnf * red_color + (1.0 - dnf) * do_color;
            gl::ClearColor(result.x / 255., result.y / 255., result.z / 255., 1.0);
        }

        Mesh::set_3d(
            &open_gl.program,
            nalgebra_glm::vec3(0.0, model_t.sin(), model_t.cos()),
            nalgebra_glm::vec2(app.screen_width as f32, app.screen_height as f32),
        );

        sun.light_dir = nalgebra_glm::vec3(0.0, model_t.sin(), model_t.cos());
    }
}

struct RenderSystem;
impl<'a> System<'a> for RenderSystem {
    type SystemData = (
        ReadStorage<'a, Renderable>,
        ReadStorage<'a, Position>,
        Read<'a, App>,
        Read<'a, MeshMgrResource>,
        Read<'a, OpenGlResource>,
        Write<'a, SunResource>,
    );

    fn run(&mut self, (render_comps, positions, app, mesh_mgr, open_gl, sun): Self::SystemData) {
        unsafe {
            gl::Viewport(0, 0, app.screen_width, app.screen_height);
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }

        open_gl.program.set();

        for (renderable, position) in (&render_comps, &positions).join() {
            match renderable.render_dist {
                Some(d) => {
                    if nalgebra_glm::length(&(position.pos - open_gl.camera.position)) > d {
                        continue;
                    }
                }
                None => {}
            }
            let mesh = mesh_mgr.data.get_mesh(renderable.mesh_id);
            renderable.texture.activate(gl::TEXTURE0);
            renderable
                .texture
                .associate_uniform(open_gl.program.id(), 0, "texture0");
            sun.depth_map.activate(gl::TEXTURE1);
            sun.depth_map
                .associate_uniform(open_gl.program.id(), 1, "shadow_map");

            let u_light_matrix = Uniform::new(open_gl.program.id(), "light_mvp").unwrap();
            let model_matrix = Mesh::get_model_matrix(position.pos, renderable.scale);
            let (light_view_matrix, light_proj_matrix) = sun.shadow_camera.gen_view_proj_matrices();
            let light_space_mvp = light_proj_matrix * light_view_matrix * model_matrix;
            unsafe {
                gl::UniformMatrix4fv(
                    u_light_matrix.id,
                    1,
                    gl::FALSE,
                    &light_space_mvp.columns(0, 4)[0],
                );
            }
            mesh.draw(
                &open_gl.program,
                &open_gl.camera,
                position.pos,
                renderable.scale,
            );
        }
    }
}

struct ShadowSystem;
impl<'a> System<'a> for ShadowSystem {
    type SystemData = (
        ReadStorage<'a, Renderable>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, CastsShadow>,
        Read<'a, MeshMgrResource>,
        Read<'a, OpenGlResource>,
        Write<'a, SunResource>,
    );

    fn run(
        &mut self,
        (render_comps, positions, shadow, mesh_mgr, open_gl, mut sun): Self::SystemData,
    ) {
        sun.fbo.bind();
        unsafe {
            gl::Viewport(0, 0, SHADOW_SIZE, SHADOW_SIZE);
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::FRONT);
            gl::Clear(gl::DEPTH_BUFFER_BIT)
        }

        // Use a simple depth shader program
        sun.shadow_program.set();

        // Compute the camera frustrum corners
        let mut frustrum = Frustrum::new(0.0, 0.999);
        frustrum.transform_points(open_gl.camera.inv_proj_view());
        let mut frustrum_2 = frustrum.clone();

        // Transform the view frustrum corners to light-space (1st time)
        sun.shadow_camera.position = nalgebra_glm::zero();
        sun.shadow_camera.lookat = sun.shadow_camera.position - sun.light_dir;
        let (light_view_matrix, _) = sun.shadow_camera.gen_view_proj_matrices();
        frustrum.transform_points(light_view_matrix);

        // Calculate an AABB for the view frustrum in light space
        let mut aabb_light_space = AABB::new();
        aabb_light_space.expand_to_fit(frustrum.points);

        // Calculate an AABB for the world, in light space
        let mut world_aabb_light_space = AABB::new();
        world_aabb_light_space.expand_to_fit([
            nalgebra_glm::zero(),
            nalgebra_glm::vec3(MAP_SIZE as f32, 0.0, 0.0),
            nalgebra_glm::vec3(0.0, MAP_SIZE as f32, 0.0),
            nalgebra_glm::vec3(MAP_SIZE as f32, MAP_SIZE as f32, 0.0),
            nalgebra_glm::vec3(0.0, 0.0, SCALE),
            nalgebra_glm::vec3(MAP_SIZE as f32, 0.0, SCALE),
            nalgebra_glm::vec3(0.0, MAP_SIZE as f32, SCALE),
            nalgebra_glm::vec3(MAP_SIZE as f32, MAP_SIZE as f32, SCALE),
        ]);
        world_aabb_light_space.transform(light_view_matrix);
        aabb_light_space.intersect_z(&world_aabb_light_space);

        // Calculate the mid-point of the near-plane on the light-frustrum
        let light_pos_light_space = aabb_light_space.pos_z_plane_midpoint();
        let light_pos_world_space =
            (nalgebra_glm::inverse(&light_view_matrix)) * light_pos_light_space;

        // Transform the view frustrum to light-space (2nd time)
        sun.shadow_camera.position = light_pos_world_space.xyz();
        sun.shadow_camera.lookat = sun.shadow_camera.position - sun.light_dir;
        let (light_view_matrix, _) = sun.shadow_camera.gen_view_proj_matrices();
        frustrum_2.transform_points(light_view_matrix);

        // Create an Orthographic Projection (2nd time)
        let mut aabb_light_space = AABB::new();
        aabb_light_space.expand_to_fit(frustrum_2.points);
        sun.shadow_camera.projection_kind = ProjectionKind::Orthographic {
            left: aabb_light_space.min.x,
            right: aabb_light_space.max.x,
            bottom: aabb_light_space.min.y,
            top: aabb_light_space.max.y,
            near: aabb_light_space.min.z,
            far: 800.0,
        };

        // Render the stuff that casts shadows
        for (renderable, position, _) in (&render_comps, &positions, &shadow).join() {
            let mesh = mesh_mgr.data.get_mesh(renderable.mesh_id);
            mesh.draw(
                &sun.shadow_program,
                &sun.shadow_camera,
                position.pos,
                renderable.scale,
            );
        }

        sun.fbo.unbind();
    }
}

struct PhysicsSystem;
impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        Read<'a, TileResource>,
    );
    fn run(&mut self, (mut positions, mut velocities, tile): Self::SystemData) {
        for (position, velocity) in (&mut positions, &mut velocities).join() {
            velocity.vel.z -= 0.005 * UNIT_PER_METER; // gravity
            position.pos += velocity.vel;

            let feet_height =
                get_z_scaled_interpolated(&tile.tiles, position.pos.x, position.pos.y);
            if position.pos.z <= feet_height {
                let normal = get_normal(&tile.tiles, position.pos.x, position.pos.y);
                let d = feet_height - position.pos.z;
                velocity.vel += normal * 0.1 * d; // normal from slopes
                if nalgebra_glm::length(&velocity.vel.xy()) < 0.1 {
                    let feet_normal = -nalgebra_glm::vec3(normal.x, normal.y, 0.0);
                    velocity.vel += feet_normal * 0.1 * d; // if standing still, remove the side-to-side component from the slope normal, so there's no slipping
                }
                // If the player is a meter deep into the earth, hard bump them
                let bump_limit = UNIT_PER_METER * 0.01;
                if feet_height - position.pos.z >= bump_limit {
                    position.pos.z = feet_height - bump_limit;
                }

                velocity.vel *= 0.8; // friction
            }
        }
    }
}

struct PlayerSystem;
impl<'a> System<'a> for PlayerSystem {
    type SystemData = (
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        WriteStorage<'a, Player>,
        Read<'a, App>,
        Write<'a, OpenGlResource>,
        Read<'a, LazyUpdate>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            mut positions,
            mut velocities,
            mut players,
            app,
            mut opengl,
            lazy,
            entities,
        ): Self::SystemData,
    ) {
        for (player, position, velocity) in (&mut players, &mut positions, &mut velocities).join() {
            // TODO: This is a lot. Can it be cleaned up somehow?
            let curr_w_state = app.keys[Scancode::W as usize];
            let curr_s_state = app.keys[Scancode::S as usize];
            let curr_a_state = app.keys[Scancode::A as usize];
            let curr_d_state = app.keys[Scancode::D as usize];
            let curr_space_state = app.keys[Scancode::Space as usize];
            let curr_shift_state = app.keys[Scancode::LShift as usize];
            let walking = curr_w_state || curr_s_state || curr_a_state || curr_d_state;
            let swimming = opengl.camera.position.z - PERSON_HEIGHT * 0.01 <= 0.5 * SCALE;
            let walk_speed: f32 = 1.0 * PERSON_HEIGHT / 62.5
                * if swimming {
                    1.0
                } else if curr_shift_state {
                    1.5
                } else {
                    1.0
                };
            let view_speed: f32 = 0.00001 * (app.screen_width as f32);
            let facing_vec = nalgebra_glm::vec3(
                player.facing.cos(),
                player.facing.sin(),
                if swimming { -player.pitch.sin() } else { 0.0 },
            );
            let sideways_vec = nalgebra_glm::cross(&opengl.camera.up, &facing_vec);
            let mut player_vel_vec: nalgebra_glm::Vec3 = nalgebra_glm::zero();
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
            if curr_space_state && (swimming || player.feet_on_ground) {
                velocity.vel.z += 0.1 * UNIT_PER_METER;
            } else if walking {
                velocity.vel += player_vel_vec.normalize() * walk_speed; // Move the player, this way moving diagonal isn't faster
            }
            player.facing -= view_speed * app.mouse_rel_x as f32;
            player.pitch = (player.pitch + view_speed * (app.mouse_rel_y as f32))
                .max(view_speed - PI / 2.0)
                .min(PI / 2.0 - view_speed);

            velocity.vel.z -= 0.005 * UNIT_PER_METER; // gravity

            opengl.camera.position = position.pos + nalgebra_glm::vec3(0.0, 0.0, PERSON_HEIGHT);

            let rot_matrix = nalgebra_glm::rotate_y(
                &nalgebra_glm::rotate_z(&nalgebra_glm::one(), player.facing),
                player.pitch,
            );
            let facing_vec = (rot_matrix * nalgebra_glm::vec4(1.0, 0.0, 0.0, 0.0)).xyz();
            opengl.camera.lookat = opengl.camera.position + facing_vec;

            if !player.prev_mouse_left_down && app.mouse_left_down {
                let bullet_entity = entities.create();
                lazy.insert(
                    bullet_entity,
                    Renderable {
                        mesh_id: 2,
                        scale: nalgebra_glm::vec3(0.01, 0.01, 0.01),
                        texture: Texture::from_png("res/tree.png"),
                        render_dist: Some(128.0),
                    },
                );
                lazy.insert(
                    bullet_entity,
                    Position {
                        pos: opengl.camera.position,
                    },
                );
                lazy.insert(
                    bullet_entity,
                    Velocity {
                        vel: 0.1 * facing_vec,
                    },
                );
                lazy.insert(bullet_entity, Projectile {});
                lazy.insert(
                    bullet_entity,
                    Collidable {
                        aabb: AABB::from_min_max(
                            nalgebra_glm::vec3(-0.005, -0.005, -0.005),
                            nalgebra_glm::vec3(0.005, 0.005, 0.005),
                        ),
                    },
                );
            }
            player.prev_mouse_left_down = app.mouse_left_down;
        }
    }
}

struct TickSystem;
impl<'a> System<'a> for TickSystem {
    type SystemData = Write<'a, TickResource>;

    fn run(&mut self, mut tick_res: Self::SystemData) {
        tick_res.t += 1.0;
    }
}

struct TreasureSystem;
impl<'a> System<'a> for TreasureSystem {
    type SystemData = (
        WriteStorage<'a, TreasureMap>,
        WriteStorage<'a, Quad>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Velocity>,
        ReadStorage<'a, Player>,
        Read<'a, OpenGlResource>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (mut treasure_maps, mut quads, positions, velocities, player, opengl, entities): Self::SystemData,
    ) {
        let (_, player_entity) = (&player, &entities).join().next().unwrap();
        let player_velocity = velocities.get(player_entity).unwrap();
        for (treasure_map, quad) in (&mut treasure_maps, &mut quads).join() {
            // Get the corresponding treasure entity
            let treasure_entity = treasure_map.treasure_entity;

            // Access components of the treasure entity
            if let Some(treasure_position) = positions.get(treasure_entity) {
                let to_treasure = treasure_position.pos - opengl.camera.position;
                if nalgebra_glm::length(&to_treasure) < 3.0 * UNIT_PER_METER {
                    treasure_map.found = true;
                }

                if treasure_map.found {
                    quad.opacity = 1.0;
                    continue;
                } else if nalgebra_glm::length(&player_velocity.vel.xy()) < 0.01 {
                    quad.opacity = 0.2;
                    continue;
                }

                let player_moving_dir = player_velocity.vel.xy().normalize();
                let to_treasure_dir = to_treasure.xy().normalize();
                let dot = player_moving_dir.dot(&to_treasure_dir);

                quad.opacity = dot.clamp(0.2, 1.0);
            }
        }
    }
}

struct MobSystem;
impl<'a> System<'a> for MobSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Mob>,
        Read<'a, OpenGlResource>,
    );

    fn run(&mut self, (positions, mut velocities, mobs, opengl): Self::SystemData) {
        for (position, velocity, _mob) in (&positions, &mut velocities, &mobs).join() {
            let to_player_dir = (opengl.camera.position - position.pos)
                .xy()
                .normalize()
                .scale(0.01);
            velocity.vel.x = to_player_dir.x;
            velocity.vel.y = to_player_dir.y;
        }
    }
}

struct ProjectileSystem;
impl<'a> System<'a> for ProjectileSystem {
    type SystemData = (
        WriteStorage<'a, Position>,
        WriteStorage<'a, Projectile>,
        Read<'a, TileResource>,
        Entities<'a>,
    );

    fn run(&mut self, (mut positions, mut projectiles, tile, entities): Self::SystemData) {
        for (position, _, entity) in (&mut positions, &mut projectiles, &entities).join() {
            let tile_z: f32 =
                get_z_scaled_interpolated(&tile.tiles, position.pos.x, position.pos.y);
            if position.pos.z < tile_z {
                entities.delete(entity).unwrap();
            }
        }
    }
}

struct CollisionSystem;
impl<'a> System<'a> for CollisionSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Projectile>,
        WriteStorage<'a, Mob>,
        ReadStorage<'a, Collidable>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (positions, mut velocities, projectiles, mut mobs, collidable, entities): Self::SystemData,
    ) {
        // Collect each projectile information
        // This is needed because Rust's borrow checker is sorta kinda awful, no cap!
        let mut projectile_data = Vec::new();
        for (proj_position, proj_collidable, _, proj_entity) in
            (&positions, &collidable, &projectiles, &entities).join()
        {
            let proj_aabb = proj_collidable.aabb.translate(proj_position.pos);
            let proj_velocity = velocities.get(proj_entity).unwrap();
            projectile_data.push((proj_aabb, proj_velocity.vel.clone(), proj_entity));
        }

        // For each mob, check if any projectile intersects it
        for (mob_position, mob_collidable, _, mob_entity) in
            (&positions, &collidable, &mut mobs, &entities).join()
        {
            let mob_aabb = mob_collidable.aabb.translate(mob_position.pos);
            let mob_velocity = velocities.get_mut(mob_entity).unwrap();
            for (proj_aabb, proj_velocity, proj_entity) in &projectile_data {
                if proj_aabb.intersects(&mob_aabb) {
                    entities.delete(*proj_entity).unwrap();
                    mob_velocity.vel.x += proj_velocity.x;
                    mob_velocity.vel.y += proj_velocity.y;
                }
            }
        }
    }
}

/*
 * SCENE STUFF
 */
pub struct Island {
    world: World,
    update_dispatcher: Dispatcher<'static, 'static>,
    render_dispatcher: Dispatcher<'static, 'static>,
    ui_render_dispatcher: Dispatcher<'static, 'static>,
}

impl Scene for Island {
    fn update(&mut self, app: &App) {
        self.world.insert((*app).clone());
        self.update_dispatcher.dispatch_seq(&mut self.world);
        self.world.maintain();
    }

    fn render(&mut self, _app: &App) {
        self.render_dispatcher.dispatch_seq(&mut self.world);
        self.ui_render_dispatcher.dispatch_seq(&mut self.world);
    }
}

impl Island {
    pub fn new() -> Self {
        // Setup ECS the world
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Renderable>();
        world.register::<Player>();
        world.register::<CastsShadow>();
        world.register::<TreasureMap>();
        world.register::<Mob>();
        world.register::<Projectile>();
        world.register::<Collidable>();

        // Setup the dispatchers
        let mut update_dispatcher_builder = DispatcherBuilder::new();
        update_dispatcher_builder.add(PlayerSystem, "player system", &[]);
        update_dispatcher_builder.add(PhysicsSystem, "physics system", &[]);
        update_dispatcher_builder.add(TickSystem, "tick system", &[]);
        update_dispatcher_builder.add(TreasureSystem, "treasure system", &[]);
        update_dispatcher_builder.add(MobSystem, "mob system", &[]);
        update_dispatcher_builder.add(ProjectileSystem, "projectile system", &[]);
        update_dispatcher_builder.add(CollisionSystem, "collision system", &[]);

        let mut render_dispatcher_builder = DispatcherBuilder::new();
        render_dispatcher_builder.add(SkySystem, "sky system", &[]);
        render_dispatcher_builder.add(ShadowSystem, "shadow system", &[]);
        render_dispatcher_builder.add(RenderSystem, "render system", &[]);

        let mut ui_render_dispatcher_builder = DispatcherBuilder::new();
        initialize_gui(&mut world, &mut ui_render_dispatcher_builder);

        // Setup island map
        let mut rng = rand::thread_rng();
        let mut map = generate(MAP_SIZE, 0.1, rng.gen());
        create_bulge(&mut map);
        erosion(&mut map, MAP_SIZE, 51.0);
        let spawn_point =
            nalgebra_glm::vec3((MAP_SIZE / 2) as f32, (MAP_SIZE / 2) as f32, 2.0 * SCALE);

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
        let mob_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(MOB_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let tree_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(CONE_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        world.insert(MeshMgrResource { data: mesh_mgr });

        let depth_map = Texture::new();
        depth_map.load_depth_buffer(SHADOW_SIZE, SHADOW_SIZE);
        let fbo = Fbo::new();
        fbo.bind();
        depth_map.post_bind();

        // Add entities
        world
            .create_entity()
            .with(Renderable {
                mesh_id: grass_mesh,
                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                texture: Texture::from_png("res/grass.png"),
                render_dist: None,
            })
            .with(Position {
                pos: nalgebra_glm::zero(),
            })
            .with(CastsShadow {})
            .build();
        world
            .create_entity()
            .with(Renderable {
                mesh_id: quad_mesh,
                scale: nalgebra_glm::vec3(1000.0, 1000.0, 1000.0),
                texture: Texture::from_png("res/water.png"),
                render_dist: None,
            })
            .with(Position {
                pos: nalgebra_glm::vec3(0.0, 0.0, SCALE * 0.5),
            })
            .build();
        world
            .create_entity()
            .with(Quad::from_text(
                "+",
                font,
                Color::RGBA(255, 255, 255, 255),
                quad_mesh,
            ))
            .build();
        for _ in 0..(MAP_SIZE / 4) {
            // Add all the trees
            let mut attempts = 0;
            loop {
                let (x, y) = (
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                );
                let height = get_z_scaled_interpolated(&map, x, y);
                let dot_prod = get_dot_prod(&map, x, y).abs();
                let variation = rng.gen_range(0.0..1.0);
                if height >= SCALE && dot_prod > 0.99 {
                    world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: tree_mesh,
                            scale: nalgebra_glm::vec3(
                                (15.0 + 30.0 * variation) * UNIT_PER_METER,
                                (15.0 + 30.0 * variation) * UNIT_PER_METER,
                                (15.0 + 30.0 * variation) * UNIT_PER_METER,
                            ),
                            texture: Texture::from_png("res/tree.png"),
                            render_dist: Some(128.0),
                        })
                        .with(Position {
                            pos: nalgebra_glm::vec3(x, y, height),
                        })
                        .with(CastsShadow {})
                        .build();
                    break;
                }
                if attempts > 100 {
                    break;
                }
                attempts += 1;
            }
        }
        const NUM_TREASURE: usize = MAP_SIZE / 51;
        for i in 0..NUM_TREASURE {
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
                    // Add treasure
                    let treasure_entity = world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: cube_mesh,
                            scale: nalgebra_glm::vec3(0.1, 0.1, 0.1),
                            texture: Texture::from_png("res/tree.png"),
                            render_dist: Some(128.0),
                        })
                        .with(Position {
                            pos: nalgebra_glm::vec3(x, y, height),
                        })
                        .with(CastsShadow {})
                        .build();
                    // Add corresponding map
                    world
                        .create_entity()
                        .with(Quad::from_texture(
                            Texture::from_png("res/map.png"),
                            nalgebra_glm::vec3(
                                (i as f32) / (NUM_TREASURE as f32 - 1.0) - 0.5,
                                0.9,
                                0.0,
                            ),
                            32,
                            32,
                            quad_mesh,
                        ))
                        .with(TreasureMap {
                            treasure_entity,
                            found: false,
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
        const NUM_MOBS: usize = MAP_SIZE;
        for _ in 0..NUM_MOBS {
            let mut attempts = 0;
            loop {
                let (x, y) = (
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_SIZE as f32 - 1.0),
                );
                let height = get_z_scaled_interpolated(&map, x, y);
                if height >= 0.5 * SCALE {
                    // Add mob
                    world
                        .create_entity()
                        .with(Renderable {
                            mesh_id: mob_mesh,
                            scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                            texture: Texture::from_png("res/tree.png"),
                            render_dist: Some(128.0),
                        })
                        .with(Position {
                            pos: nalgebra_glm::vec3(x, y, height),
                        })
                        .with(Velocity {
                            vel: nalgebra_glm::zero(),
                        })
                        .with(CastsShadow {})
                        .with(Mob {})
                        .with(Collidable {
                            aabb: AABB::from_min_max(
                                nalgebra_glm::vec3(-0.05, -0.05, 0.0),
                                nalgebra_glm::vec3(0.05, 0.05, 0.2),
                            ),
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
        // Add the player
        world
            .create_entity()
            .with(Renderable {
                mesh_id: mob_mesh,
                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                texture: Texture::from_png("res/tree.png"),
                render_dist: Some(128.0),
            })
            .with(CastsShadow {})
            .with(Player {
                feet_on_ground: true,
                facing: 3.14,
                pitch: 0.0,
                prev_mouse_left_down: false,
            })
            .with(Position { pos: spawn_point })
            .with(Velocity {
                vel: nalgebra_glm::zero(),
            })
            .build();

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
            program: create_program(
                include_str!("../shaders/3d.vert"),
                include_str!("../shaders/3d.frag"),
            )
            .unwrap(),
        });
        world.insert(UIResource {
            camera: Camera::new(
                nalgebra_glm::vec3(0.0, 0.0, 1.0),
                nalgebra_glm::zero(),
                nalgebra_glm::vec3(0.0, 1.0, 0.0),
                ProjectionKind::Orthographic {
                    left: -1.0,
                    right: 1.0,
                    bottom: -1.0,
                    top: 1.0,
                    near: 0.01,
                    far: 10.0,
                },
            ),
            program: create_program(
                include_str!("../shaders/2d.vert"),
                include_str!("../shaders/2d.frag"),
            )
            .unwrap(),
        });
        world.insert(TileResource { tiles: map });
        let sun_scale = 30.0;
        world.insert(SunResource {
            shadow_camera: Camera::new(
                nalgebra_glm::vec3(MAP_SIZE as f32 / -2.0, 0.0, SCALE * 2.0),
                nalgebra_glm::vec3(MAP_SIZE as f32 / 2.0, MAP_SIZE as f32 / 2.0, SCALE * 0.5),
                nalgebra_glm::vec3(0.0, 0.0, 1.0),
                ProjectionKind::Orthographic {
                    left: -sun_scale,
                    right: sun_scale,
                    bottom: -sun_scale,
                    top: sun_scale,
                    near: 0.01,
                    far: 5000.0,
                },
            ),
            shadow_program: create_program(
                include_str!("../shaders/shadow.vert"),
                include_str!("../shaders/shadow.frag"),
            )
            .unwrap(),
            fbo,
            depth_map,
            light_dir: nalgebra_glm::vec3(0.0, 0.0, 1.0),
        });

        Self {
            world,
            update_dispatcher: update_dispatcher_builder.build(),
            render_dispatcher: render_dispatcher_builder.build(),
            ui_render_dispatcher: ui_render_dispatcher_builder.build(),
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
