use std::{f32::consts::PI, time::Instant};

use rand::{Rng, SeedableRng};
use sdl2::{keyboard::Scancode, pixels::Color};
use specs::{prelude::*, Component, Join, ReadStorage};

use crate::{
    engine::{
        aabb::AABB,
        audio::{AudioManager, AudioResource},
        camera::{Camera, ProjectionKind},
        objects::{create_program, Texture},
        perlin::{PerlinMap, PerlinMapResource},
        physics::{PositionComponent, VelocityComponent},
        render3d::{Mesh, MeshComponent, MeshMgr, MeshMgrResource, OpenGlResource, Render3dSystem},
        shadow_map::{CastsShadowComponent, ShadowSystem, SunResource},
        text::{initialize_gui, FontMgr, QuadComponent, UIResource},
    },
    App, Scene,
};

const MAP_WIDTH: usize = 400;
const CHUNK_SIZE: usize = 64;
const UNIT_PER_METER: f32 = 0.05;
const PERSON_HEIGHT: f32 = 1.6764 * UNIT_PER_METER;

pub const QUAD_DATA: &[u8] = include_bytes!("../../res/quad.obj");
pub const CONE_DATA: &[u8] = include_bytes!("../../res/cone.obj");
pub const BUSH_DATA: &[u8] = include_bytes!("../../res/bush.obj");
pub const CUBE_DATA: &[u8] = include_bytes!("../../res/cube.obj");
pub const MOB_DATA: &[u8] = include_bytes!("../../res/mob.obj");
pub const CHEST_DATA: &[u8] = include_bytes!("../../res/chest.obj");

/*
 * COMPONENTS
 */
#[derive(Component)]
#[storage(HashMapStorage)]
struct PlayerComponent {
    // Status
    feet_on_ground: bool,

    // View variables
    facing: f32,
    pitch: f32,

    // Animations and timing
    t_last_shot: usize,
    t_last_walk_played: usize,
}

#[derive(Component)]
#[storage(VecStorage)]
struct TreasureMapComponent {
    treasure_entity: Entity,
    found: bool,
}

#[derive(Component)]
#[storage(VecStorage)]
struct MobComponent {}

#[derive(Component)]
#[storage(VecStorage)]
struct ProjectileComponent {}

#[derive(Component)]
#[storage(VecStorage)]
struct CollidableComponent {
    aabb: AABB,
}

#[derive(Component)]
#[storage(VecStorage)]
struct HealthComponent {
    health: f32, // 1.0 is full health, 0.0 is dead
}

#[derive(Component)]
#[storage(VecStorage)]
struct CylinderRadiusComponent {
    radius: f32, // 1.0 is full health, 0.0 is dead
}

#[derive(Component)]
#[storage(VecStorage)]
struct DeathSplishAnimComponent {
    timeline: f32, // 0.0 is just starting 1.0 is end
}

/*
 * SYSTEMS
 */
struct SkySystem;
impl<'a> System<'a> for SkySystem {
    type SystemData = (
        Read<'a, App>,
        Read<'a, OpenGlResource>,
        Write<'a, SunResource>,
    );
    fn run(&mut self, (app, open_gl, mut sun): Self::SystemData) {
        const MIN_PER_DAY: f32 = 60.0;
        // Noon:     0.0
        // Evening:  1.57
        // Midnight: 3.14
        // Morning:  4.71
        // Noon2:    6.28
        let model_t = app.ticks as f32 / (MIN_PER_DAY * 60.0 * 62.6) + 5.5;
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

struct PhysicsSystem;
impl<'a> System<'a> for PhysicsSystem {
    type SystemData = (
        WriteStorage<'a, PositionComponent>,
        WriteStorage<'a, VelocityComponent>,
        Read<'a, PerlinMapResource>,
    );
    fn run(&mut self, (mut positions, mut velocities, tile): Self::SystemData) {
        for (position, velocity) in (&mut positions, &mut velocities).join() {
            velocity.vel.z -= 0.005 * UNIT_PER_METER; // gravity
            position.pos += velocity.vel;

            let feet_height = tile.map.get_z_interpolated(position.pos.xy());
            if position.pos.z <= feet_height {
                let normal = tile.map.get_normal(position.pos.xy());
                let d = feet_height - position.pos.z;
                velocity.vel += normal * 0.1 * d; // normal from slopes
                if nalgebra_glm::length(&velocity.vel.xy()) < 0.05 {
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
        WriteStorage<'a, PositionComponent>,
        WriteStorage<'a, VelocityComponent>,
        WriteStorage<'a, PlayerComponent>,
        Read<'a, App>,
        Write<'a, OpenGlResource>,
        Read<'a, AudioResource>,
        Read<'a, PerlinMapResource>,
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
            audio,
            tiles,
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
            let swimming = position.pos.z <= 0.5;
            let walk_speed: f32 = if swimming {
                1.0
            } else if curr_shift_state {
                1.3
            } else {
                1.0
            };
            let view_speed: f32 = 0.01;
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
            if curr_space_state && swimming {
                velocity.vel.z += 0.001 * UNIT_PER_METER;
                velocity.vel.z = velocity.vel.z.min(0.1);
            } else if curr_space_state && player.feet_on_ground {
                velocity.vel.z += 0.1 * UNIT_PER_METER;
                audio.audio_mgr.play_sound("res/jump.ogg".to_string(), 128);
                println!("{}", opengl.camera.position);
            } else if walking {
                // Move the player, this way moving diagonal isn't faster
                velocity.vel +=
                    player_vel_vec.normalize() * walk_speed * 4.317 * UNIT_PER_METER / 62.5;
            }
            player.facing -= view_speed * app.mouse_rel_x as f32;
            player.pitch = (player.pitch + view_speed * (app.mouse_rel_y as f32))
                .max(view_speed - PI / 2.0)
                .min(PI / 2.0 - view_speed);

            opengl.camera.position = position.pos + nalgebra_glm::vec3(0.0, 0.0, PERSON_HEIGHT);

            let feet_height = tiles.map.get_z_interpolated(opengl.camera.position.xy());
            player.feet_on_ground = opengl.camera.position.z - PERSON_HEIGHT <= feet_height;
            if !player.feet_on_ground {
                velocity.vel.x *= 0.8;
                velocity.vel.y *= 0.8;
            }

            let rot_matrix = nalgebra_glm::rotate_y(
                &nalgebra_glm::rotate_z(&nalgebra_glm::one(), player.facing),
                player.pitch,
            );
            let facing_vec = (rot_matrix * nalgebra_glm::vec4(1.0, 0.0, 0.0, 0.0)).xyz();
            opengl.camera.lookat = opengl.camera.position + facing_vec;

            const SHOT_PERIOD: usize = 7;
            const SHOT_VEL: f32 = 74.0; // m/s
            if app.ticks - player.t_last_shot > SHOT_PERIOD && app.mouse_left_down {
                player.t_last_shot = app.ticks;
                let gun_pos =
                    opengl.camera.position + nalgebra_glm::vec3(0.0, 0.0, -0.5 * UNIT_PER_METER);
                let convergence = ((opengl.camera.position + facing_vec * 1.0) - gun_pos)
                    .normalize()
                    .scale(SHOT_VEL * UNIT_PER_METER / 62.5);
                let bullet_entity = entities.create();
                lazy.insert(
                    bullet_entity,
                    MeshComponent {
                        mesh_id: 1,
                        scale: nalgebra_glm::vec3(0.01, 0.01, 0.01),
                        texture: Texture::from_png("res/bullet.png"),
                        render_dist: Some(128.0),
                    },
                );
                lazy.insert(bullet_entity, PositionComponent { pos: gun_pos });
                lazy.insert(bullet_entity, VelocityComponent { vel: convergence });
                lazy.insert(bullet_entity, ProjectileComponent {});
                lazy.insert(
                    bullet_entity,
                    CollidableComponent {
                        aabb: AABB::from_min_max(
                            nalgebra_glm::vec3(-0.005, -0.005, -0.005),
                            nalgebra_glm::vec3(0.005, 0.005, 0.005),
                        ),
                    },
                );
                audio.audio_mgr.play_sound("res/pop.ogg".to_string(), 128);
            }
            // 107 steps per minute
            // 60 seconds per 107 steps
            // 0.56 seconds per step
            // 35 ticks per step
            if walking
                && player.feet_on_ground
                && (app.ticks - player.t_last_walk_played) as f32 > 35.0 / walk_speed
            {
                player.t_last_walk_played = app.ticks;
                audio.audio_mgr.play_sound("res/walk.ogg".to_string(), 35);
            }
        }
    }
}

struct TreasureSystem;
impl<'a> System<'a> for TreasureSystem {
    type SystemData = (
        WriteStorage<'a, TreasureMapComponent>,
        WriteStorage<'a, QuadComponent>,
        ReadStorage<'a, PositionComponent>,
        ReadStorage<'a, VelocityComponent>,
        ReadStorage<'a, PlayerComponent>,
        Read<'a, OpenGlResource>,
        Read<'a, AudioResource>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            mut treasure_maps,
            mut quads,
            positions,
            velocities,
            player,
            opengl,
            audio,
            entities,
        ): Self::SystemData,
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
                    if !treasure_map.found {
                        quad.texture = Texture::from_png("res/gold.png");
                        audio.audio_mgr.play_sound("res/win.ogg".to_string(), 128);
                    }
                    treasure_map.found = true;
                }

                if treasure_map.found {
                    quad.opacity = 1.0;
                    continue;
                } else if nalgebra_glm::length(&player_velocity.vel.xy()) < 0.001 {
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
        ReadStorage<'a, PositionComponent>,
        WriteStorage<'a, VelocityComponent>,
        ReadStorage<'a, MobComponent>,
        Read<'a, OpenGlResource>,
    );

    fn run(&mut self, (positions, mut velocities, mobs, opengl): Self::SystemData) {
        for (position, velocity, _mob) in (&positions, &mut velocities, &mobs).join() {
            let to_player = (opengl.camera.position - position.pos).xy();
            if nalgebra_glm::length(&to_player) > 4.0 {
                continue;
            }
            let to_player_dir = to_player.normalize().scale(0.01);
            velocity.vel.x = to_player_dir.x;
            velocity.vel.y = to_player_dir.y;
        }
    }
}

struct ProjectileSystem;
impl<'a> System<'a> for ProjectileSystem {
    type SystemData = (
        WriteStorage<'a, PositionComponent>,
        WriteStorage<'a, ProjectileComponent>,
        Read<'a, PerlinMapResource>,
        Read<'a, AudioResource>,
        Read<'a, OpenGlResource>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (mut positions, mut projectiles, tile, audio, opengl, entities): Self::SystemData,
    ) {
        for (position, _, entity) in (&mut positions, &mut projectiles, &entities).join() {
            let tile_z: f32 = tile.map.get_z_interpolated(position.pos.xy());
            if position.pos.z < tile_z {
                entities.delete(entity).unwrap();
                let distance = nalgebra_glm::length(&(opengl.camera.position - position.pos));
                audio.audio_mgr.play_sound(
                    "res/ground.ogg".to_string(),
                    (50.0 * 128.0 / distance.powf(2.0)) as i32,
                );
            }
        }
    }
}

struct CollisionSystem;
impl<'a> System<'a> for CollisionSystem {
    type SystemData = (
        ReadStorage<'a, PositionComponent>,
        WriteStorage<'a, VelocityComponent>,
        WriteStorage<'a, HealthComponent>,
        ReadStorage<'a, ProjectileComponent>,
        ReadStorage<'a, MobComponent>,
        ReadStorage<'a, CollidableComponent>,
        Read<'a, PerlinMapResource>,
        Read<'a, AudioResource>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            positions,
            mut velocities,
            mut healths,
            projectiles,
            mobs,
            collidable,
            tiles,
            audio,
            entities,
        ): Self::SystemData,
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
        for (mob_position, mob_health, mob_collidable, _, mob_entity) in
            (&positions, &mut healths, &collidable, &mobs, &entities).join()
        {
            let mob_aabb = mob_collidable.aabb.translate(mob_position.pos);
            let mob_velocity = velocities.get_mut(mob_entity).unwrap();
            for (proj_aabb, proj_velocity, proj_entity) in &projectile_data {
                if proj_aabb.intersects(&mob_aabb) {
                    entities.delete(*proj_entity).unwrap();
                    mob_velocity.vel.x += proj_velocity.x;
                    mob_velocity.vel.y += proj_velocity.y;
                    let tile_z: f32 = tiles.map.get_z_interpolated(mob_position.pos.xy());
                    if mob_position.pos.z + 0.01 <= tile_z {
                        mob_velocity.vel.z += 0.1 * UNIT_PER_METER;
                    }
                    mob_health.health -= 0.1;
                    audio.audio_mgr.play_sound("res/hit.ogg".to_string(), 128);
                }
            }
        }
    }
}

struct HealthSystem;
impl<'a> System<'a> for HealthSystem {
    type SystemData = WriteStorage<'a, HealthComponent>;

    fn run(&mut self, mut healths: Self::SystemData) {
        for health in (&mut healths).join() {
            health.health = health.health.clamp(0.0, 1.0);
        }
    }
}

struct MobDeathSystem;
impl<'a> System<'a> for MobDeathSystem {
    type SystemData = (
        WriteStorage<'a, HealthComponent>,
        ReadStorage<'a, MobComponent>,
        WriteStorage<'a, DeathSplishAnimComponent>,
        WriteStorage<'a, CollidableComponent>,
        WriteStorage<'a, CastsShadowComponent>,
        Read<'a, AudioResource>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            mut healths,
            mobs,
            mut death_splish_anims,
            mut collidables,
            mut casts_shadows,
            audio,
            entities,
        ): Self::SystemData,
    ) {
        let mut removed_entities = Vec::new();
        for (health, _mob, entity) in (&healths, &mobs, &entities).join() {
            if health.health <= 0.0 {
                death_splish_anims
                    .insert(entity, DeathSplishAnimComponent { timeline: 0.0 })
                    .unwrap();
                removed_entities.push(entity);
            }
        }
        for removed_entity in removed_entities {
            healths.remove(removed_entity);
            collidables.remove(removed_entity);
            casts_shadows.remove(removed_entity);
            audio.audio_mgr.play_sound("res/dead.ogg".to_string(), 128);
        }
    }
}

struct DeathSplishAnimSystem;
impl<'a> System<'a> for DeathSplishAnimSystem {
    type SystemData = (
        WriteStorage<'a, MeshComponent>,
        WriteStorage<'a, DeathSplishAnimComponent>,
        Entities<'a>,
    );

    fn run(&mut self, (mut renderables, mut death_splish_anims, entities): Self::SystemData) {
        let mut removed_entities = Vec::new();
        for (renderable, death_splish_anim, entity) in
            (&mut renderables, &mut death_splish_anims, &entities).join()
        {
            death_splish_anim.timeline += 1.0 / (1.0 * 62.0);
            let z = 1.0 - death_splish_anim.timeline.powf(2.0);
            let xy = (3.33 / (z + 0.833)).sqrt();
            renderable.scale = nalgebra_glm::vec3(xy, xy, z);
            if death_splish_anim.timeline >= 1.0 {
                removed_entities.push(entity);
            }
        }
        for removed_entity in removed_entities {
            entities.delete(removed_entity).unwrap();
        }
    }
}

struct CylindricalCollisionSystem;
impl<'a> System<'a> for CylindricalCollisionSystem {
    type SystemData = (
        ReadStorage<'a, CylinderRadiusComponent>,
        ReadStorage<'a, PositionComponent>,
        WriteStorage<'a, VelocityComponent>,
        Entities<'a>,
    );

    fn run(&mut self, (cyl_radii, positions, mut velocities, entities): Self::SystemData) {
        // let mut cyl_data = Vec::new();
        // for (cyl_radius, cyl_position, cyl_entity) in (&cyl_radii, &positions, &entities).join() {
        //     cyl_data.push((cyl_radius.radius, cyl_position.pos.clone(), cyl_entity));
        // }

        // for (cyl_radius, cyl_position, cyl_velocity, cyl_entity) in
        //     (&cyl_radii, &positions, &mut velocities, &entities).join()
        // {
        //     for data in &cyl_data {
        //         if data.2 == cyl_entity {
        //             continue;
        //         }
        //         let from_cyl = cyl_position.pos - data.1;
        //         if nalgebra_glm::length(&from_cyl.xy()) <= cyl_radius.radius + data.0 {
        //             let bounce_impulse = from_cyl.xy().scale(0.05);
        //             cyl_velocity.vel.x += bounce_impulse.x;
        //             cyl_velocity.vel.y += bounce_impulse.y;
        //         }
        //     }
        // }
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
        world.register::<PositionComponent>();
        world.register::<VelocityComponent>();
        world.register::<MeshComponent>();
        world.register::<PlayerComponent>();
        world.register::<CastsShadowComponent>();
        world.register::<TreasureMapComponent>();
        world.register::<MobComponent>();
        world.register::<ProjectileComponent>();
        world.register::<CollidableComponent>();
        world.register::<HealthComponent>();
        world.register::<CylinderRadiusComponent>();
        world.register::<DeathSplishAnimComponent>();

        // Setup the dispatchers
        let mut update_dispatcher_builder = DispatcherBuilder::new();
        update_dispatcher_builder.add(PlayerSystem, "player system", &[]);
        update_dispatcher_builder.add(CylindricalCollisionSystem, "cylinder collision system", &[]);
        update_dispatcher_builder.add(PhysicsSystem, "physics system", &[]);
        update_dispatcher_builder.add(TreasureSystem, "treasure system", &[]);
        update_dispatcher_builder.add(MobSystem, "mob system", &[]);
        update_dispatcher_builder.add(ProjectileSystem, "projectile system", &[]);
        update_dispatcher_builder.add(CollisionSystem, "collision system", &[]);
        update_dispatcher_builder.add(HealthSystem, "health system", &[]);
        update_dispatcher_builder.add(MobDeathSystem, "mobe deat system", &[]);
        update_dispatcher_builder.add(DeathSplishAnimSystem, "deat spih ah system", &[]);

        let mut render_dispatcher_builder = DispatcherBuilder::new();
        render_dispatcher_builder.add(SkySystem, "sky system", &[]);
        render_dispatcher_builder.add(ShadowSystem, "shadow system", &[]);
        render_dispatcher_builder.add(Render3dSystem, "render system", &[]);

        let mut ui_render_dispatcher_builder = DispatcherBuilder::new();
        initialize_gui(&mut world, &mut ui_render_dispatcher_builder);

        // Setup island map
        println!("Setting up island...");
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut map = PerlinMap::new(MAP_WIDTH, 0.03, rng.gen(), 1.0);
        // map.normalize();

        println!("Creating bulge...");
        map.normalize();
        map.create_bulge();

        println!("Eroding...");
        let start = Instant::now();
        map.erode(20_000, rng.gen());
        println!("Erode time: {:?}", start.elapsed());

        let height = map.get_z_interpolated(nalgebra_glm::vec2(
            (MAP_WIDTH / 2) as f32,
            (MAP_WIDTH / 2) as f32,
        ));
        let mut spawn_point =
            nalgebra_glm::vec3((MAP_WIDTH / 2) as f32, (MAP_WIDTH / 2) as f32, height);
        for y in 0..MAP_WIDTH / 2 {
            let height = map.get_z_interpolated(nalgebra_glm::vec2(
                (MAP_WIDTH / 2) as f32,
                (y + MAP_WIDTH / 2) as f32,
            ));
            if height >= 0.5 {
                spawn_point =
                    nalgebra_glm::vec3((MAP_WIDTH / 2) as f32, (y + MAP_WIDTH / 2) as f32, height);
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
        let quad_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(QUAD_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let _cube_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(CUBE_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let mob_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(MOB_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let tree_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(CONE_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let bush_mesh =
            mesh_mgr.add_mesh(Mesh::from_obj(BUSH_DATA, nalgebra_glm::vec3(1.0, 1.0, 1.0)));
        let chest_mesh = mesh_mgr.add_mesh(Mesh::from_obj(
            CHEST_DATA,
            nalgebra_glm::vec3(1.0, 1.0, 1.0),
        ));

        // Add entities
        for chunk_y in (0..(MAP_WIDTH)).step_by(CHUNK_SIZE) {
            for chunk_x in (0..(MAP_WIDTH)).step_by(CHUNK_SIZE) {
                let (i, v, n, u, c) = create_mesh(&map, chunk_x, chunk_y);
                let grass_mesh = mesh_mgr.add_mesh(Mesh::new(i, vec![v, n, u, c]));
                world
                    .create_entity()
                    .with(MeshComponent {
                        mesh_id: grass_mesh,
                        scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                        texture: Texture::from_png("res/grass.png"),
                        render_dist: Some(CHUNK_SIZE as f32 * 4.0),
                    })
                    .with(PositionComponent {
                        pos: nalgebra_glm::vec3(chunk_x as f32, chunk_y as f32, 0.0),
                    })
                    .with(CastsShadowComponent {})
                    .build();
            }
        }
        world.insert(MeshMgrResource { data: mesh_mgr });
        world
            .create_entity()
            .with(MeshComponent {
                mesh_id: quad_mesh,
                scale: nalgebra_glm::vec3(1000.0, 1000.0, 1000.0),
                texture: Texture::from_png("res/water.png"),
                render_dist: None,
            })
            .with(PositionComponent {
                pos: nalgebra_glm::vec3(0.0, 0.0, 0.5),
            })
            .build();
        world
            .create_entity()
            .with(QuadComponent::from_text(
                "+",
                &font,
                Color::RGBA(255, 255, 255, 255),
                quad_mesh,
            ))
            .build();
        world
            .create_entity()
            .with(QuadComponent::from_text(
                "Collect all maps to win!",
                &font,
                Color::RGBA(255, 255, 255, 255),
                quad_mesh,
            ))
            .build();
        for _ in 0..(MAP_WIDTH * 4) {
            // Add all the trees
            let mut attempts = 0;
            loop {
                let pos = nalgebra_glm::vec2(
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                );
                let height = map.get_z_interpolated(pos);
                let dot_prod = map.get_dot_prod(pos).abs();
                let variation = rng.gen_range(0.0..1.0);
                let vegatation = map.flow(pos);
                let scale = (15.0 + 70.0 * variation) * UNIT_PER_METER;
                if height >= 1.0 && dot_prod > 0.99 && vegatation > 20.0 {
                    world
                        .create_entity()
                        .with(MeshComponent {
                            mesh_id: tree_mesh,
                            scale: nalgebra_glm::vec3(scale, scale, scale),
                            texture: Texture::from_png("res/tree.png"),
                            render_dist: Some(CHUNK_SIZE as f32 * 4.0),
                        })
                        .with(PositionComponent {
                            pos: nalgebra_glm::vec3(pos.x, pos.y, height),
                        })
                        .with(CastsShadowComponent {})
                        .with(CylinderRadiusComponent {
                            radius: 0.06 * scale,
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
        for _ in 0..(MAP_WIDTH * 2) {
            // Add all the bushes
            let mut attempts = 0;
            loop {
                let pos = nalgebra_glm::vec2(
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                );
                let height = map.get_z_interpolated(pos);
                let dot_prod = map.get_dot_prod(pos).abs();
                let variation = rng.gen_range(0.0..1.0);
                if height >= 0.66 && dot_prod >= 0.8 && dot_prod <= 0.9
                //  && map.flow(pos) > 1.0
                {
                    world
                        .create_entity()
                        .with(MeshComponent {
                            mesh_id: bush_mesh,
                            scale: nalgebra_glm::vec3(
                                (3.5 + 7.0 * variation) * UNIT_PER_METER,
                                (3.5 + 7.0 * variation) * UNIT_PER_METER,
                                (3.5 + 7.0 * variation) * UNIT_PER_METER,
                            ),
                            texture: Texture::from_png("res/tree.png"),
                            render_dist: Some(CHUNK_SIZE as f32 * 2.0),
                        })
                        .with(PositionComponent {
                            pos: nalgebra_glm::vec3(pos.x, pos.y, height),
                        })
                        .with(CastsShadowComponent {})
                        .build();
                    break;
                }
                if attempts > 100 {
                    break;
                }
                attempts += 1;
            }
        }
        const NUM_TREASURE: usize = MAP_WIDTH / 51;
        for i in 0..NUM_TREASURE {
            // Add all the treasure boxes
            let mut attempts = 0;
            loop {
                let pos = nalgebra_glm::vec2(
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                    rng.gen::<f32>() * (MAP_WIDTH as f32 - 1.0),
                );
                let height = map.get_z_interpolated(pos);
                let dot_prod = map.get_dot_prod(pos).abs();
                if height >= 0.5 && height <= 0.8 && height < 0.75 * dot_prod {
                    // Add treasure
                    let treasure_entity = world
                        .create_entity()
                        .with(MeshComponent {
                            mesh_id: chest_mesh,
                            scale: nalgebra_glm::vec3(0.05, 0.05, 0.05),
                            texture: Texture::from_png("res/chest.png"),
                            render_dist: Some(CHUNK_SIZE as f32 * 2.0),
                        })
                        .with(PositionComponent {
                            pos: nalgebra_glm::vec3(pos.x, pos.y, height),
                        })
                        .with(CastsShadowComponent {})
                        .build();
                    // Add corresponding map
                    world
                        .create_entity()
                        .with(QuadComponent::from_texture(
                            Texture::from_png("res/map.png"),
                            32,
                            32,
                            quad_mesh,
                        ))
                        .with(PositionComponent {
                            pos: nalgebra_glm::vec3(
                                (i as f32) / (NUM_TREASURE as f32 - 1.0) - 0.5,
                                0.9,
                                0.0,
                            ),
                        })
                        .with(TreasureMapComponent {
                            treasure_entity,
                            found: false,
                        })
                        .build();

                    // Add mobs
                    const NUM_MOBS: usize = 5;
                    for _ in 0..NUM_MOBS {
                        let (x, y) = (
                            rng.gen::<f32>() - 0.5 + pos.x,
                            rng.gen::<f32>() - 0.5 + pos.y,
                        );
                        world
                            .create_entity()
                            .with(MeshComponent {
                                mesh_id: mob_mesh,
                                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                                texture: Texture::from_png("res/ghost.png"),
                                render_dist: Some(CHUNK_SIZE as f32 * 2.0),
                            })
                            .with(PositionComponent {
                                pos: nalgebra_glm::vec3(x, y, height),
                            })
                            .with(VelocityComponent {
                                vel: nalgebra_glm::zero(),
                            })
                            .with(CastsShadowComponent {})
                            .with(MobComponent {})
                            .with(CollidableComponent {
                                aabb: AABB::from_min_max(
                                    nalgebra_glm::vec3(-0.05, -0.05, 0.0),
                                    nalgebra_glm::vec3(0.05, 0.05, 0.2),
                                ),
                            })
                            .with(HealthComponent { health: 1.0 })
                            .with(CylinderRadiusComponent { radius: 0.05 })
                            .build();
                    }
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
            .with(MeshComponent {
                mesh_id: mob_mesh,
                scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                texture: Texture::from_png("res/tree.png"),
                render_dist: Some(-1.0),
            })
            .with(CastsShadowComponent {})
            .with(PlayerComponent {
                feet_on_ground: true,
                facing: 3.14,
                pitch: 0.0,
                t_last_shot: 0,
                t_last_walk_played: 0,
            })
            .with(PositionComponent { pos: spawn_point })
            .with(VelocityComponent {
                vel: nalgebra_glm::zero(),
            })
            .with(CylinderRadiusComponent { radius: 0.03 })
            .build();

        // Add resources
        world.insert(App::default());
        world.insert(AudioResource {
            audio_mgr: AudioManager::new(),
        });
        world.insert(OpenGlResource {
            camera: Camera::new(
                spawn_point,
                nalgebra_glm::vec3(MAP_WIDTH as f32 / 2.0, MAP_WIDTH as f32 / 2.0, 0.5),
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
        world.insert(PerlinMapResource { map });
        let sun_scale = 30.0;
        world.insert(SunResource::new(
            Camera::new(
                nalgebra_glm::vec3(MAP_WIDTH as f32 / -2.0, 0.0, 2.0),
                nalgebra_glm::vec3(MAP_WIDTH as f32 / 2.0, MAP_WIDTH as f32 / 2.0, 0.5),
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
            create_program(
                include_str!("../shaders/shadow.vert"),
                include_str!("../shaders/shadow.frag"),
            )
            .unwrap(),
            nalgebra_glm::vec3(0.0, 0.0, 1.0),
        ));

        Self {
            world,
            update_dispatcher: update_dispatcher_builder.build(),
            render_dispatcher: render_dispatcher_builder.build(),
            ui_render_dispatcher: ui_render_dispatcher_builder.build(),
        }
    }
}

fn create_mesh(
    map: &PerlinMap,
    chunk_x: usize,
    chunk_y: usize,
) -> (Vec<u32>, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut indices = Vec::<u32>::new();
    let mut vertices = Vec::<f32>::new();
    let mut normals = Vec::<f32>::new();
    let mut uv = Vec::<f32>::new();
    let mut colors = Vec::<f32>::new();

    let mut i = 0;
    for y in 0..CHUNK_SIZE {
        let y = y + chunk_y;
        for x in 0..CHUNK_SIZE {
            let x = x + chunk_x;
            // Left triangle |\
            let offsets = vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
            add_triangle(
                map,
                &mut indices,
                &mut vertices,
                &mut normals,
                &mut uv,
                &mut colors,
                x as f32,
                y as f32,
                chunk_x as f32,
                chunk_y as f32,
                &offsets,
                &mut i,
            );

            // Right triangle \|
            let offsets = vec![(1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
            add_triangle(
                map,
                &mut indices,
                &mut vertices,
                &mut normals,
                &mut uv,
                &mut colors,
                x as f32,
                y as f32,
                chunk_x as f32,
                chunk_y as f32,
                &offsets,
                &mut i,
            );
        }
    }

    (indices, vertices, normals, uv, colors)
}

fn add_triangle(
    tiles: &PerlinMap,
    indices: &mut Vec<u32>,
    vertices: &mut Vec<f32>,
    normals: &mut Vec<f32>,
    uv: &mut Vec<f32>,
    colors: &mut Vec<f32>,
    x: f32,
    y: f32,
    chunk_x: f32,
    chunk_y: f32,
    offsets: &Vec<(f32, f32)>,
    i: &mut u32,
) {
    let mut sum_z = 0.0;
    let tri_verts: Vec<nalgebra_glm::Vec3> = offsets
        .iter()
        .map(|(xo, yo)| {
            let z = tiles.height(nalgebra_glm::vec2(x + xo, y + yo));
            let mapval = nalgebra_glm::vec3(x + xo, y + yo, z);
            sum_z += tiles.height(nalgebra_glm::vec2(x + xo, y + yo));
            add_vertex(vertices, x + xo - chunk_x, y + yo - chunk_y, z);
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
        if avg_z < 0.5 || (avg_z < 0.9 * dot_prod && 0.9 < dot_prod) {
            // sand
            colors.push(0.86);
            colors.push(0.74);
            colors.push(0.62);
        } else if dot_prod < 0.9 {
            // stone
            colors.push(0.5);
            colors.push(0.45);
            colors.push(0.4);
        } else {
            // grass
            colors.push(0.27);
            colors.push(0.36);
            colors.push(0.19);
        }
    }
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
