use specs::{Component, Join, NullStorage, Read, ReadStorage, System, Write};

use super::{
    aabb::AABB,
    camera::{Camera, ProjectionKind},
    frustrum::Frustrum,
    objects::{Fbo, Program, Texture},
    physics::PositionComponent,
    render3d::{MeshComponent, MeshMgrResource, OpenGlResource},
};

const SHADOW_SIZE: i32 = 1024;

#[derive(Default)]
pub struct SunResource {
    pub shadow_camera: Camera,
    pub shadow_program: Program,
    pub fbo: Fbo,
    pub depth_map: Texture,
    pub light_dir: nalgebra_glm::Vec3,
}

impl SunResource {
    pub fn new(
        shadow_camera: Camera,
        shadow_program: Program,
        light_dir: nalgebra_glm::Vec3,
    ) -> Self {
        let depth_map = Texture::new();
        depth_map.load_depth_buffer(SHADOW_SIZE, SHADOW_SIZE);
        let fbo = Fbo::new();
        fbo.bind();
        depth_map.post_bind();
        Self {
            shadow_camera,
            shadow_program,
            fbo,
            depth_map,
            light_dir,
        }
    }
}

#[derive(Default)]
pub struct CastsShadowComponent;
impl Component for CastsShadowComponent {
    type Storage = NullStorage<Self>;
}

pub struct ShadowSystem;
impl<'a> System<'a> for ShadowSystem {
    type SystemData = (
        ReadStorage<'a, MeshComponent>,
        ReadStorage<'a, PositionComponent>,
        ReadStorage<'a, CastsShadowComponent>,
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
        // world_aabb_light_space.expand_to_fit([
        //     nalgebra_glm::zero(),
        //     nalgebra_glm::vec3(CHUNK_SIZE as f32 * 2.0, 0.0, 0.0),
        //     nalgebra_glm::vec3(0.0, CHUNK_SIZE as f32 * 2.0, 0.0),
        //     nalgebra_glm::vec3(CHUNK_SIZE as f32 * 2.0, CHUNK_SIZE as f32 * 2.0, 0.0),
        //     nalgebra_glm::vec3(0.0, 0.0, SCALE),
        //     nalgebra_glm::vec3(CHUNK_SIZE as f32 * 2.0, 0.0, SCALE),
        //     nalgebra_glm::vec3(0.0, CHUNK_SIZE as f32 * 2.0, SCALE),
        //     nalgebra_glm::vec3(CHUNK_SIZE as f32 * 2.0, CHUNK_SIZE as f32 * 2.0, SCALE),
        // ]);
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
            match renderable.render_dist {
                Some(d) => {
                    if nalgebra_glm::length(&(position.pos - open_gl.camera.position)) > d {
                        continue;
                    }
                }
                None => {}
            }

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
