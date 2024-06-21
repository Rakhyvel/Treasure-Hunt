use crate::App;

use super::{camera::Camera, objects::*, physics::PositionComponent, shadow_map::SunResource};

use obj::{load_obj, Obj, TexturedVertex};
use specs::{Component, DenseVecStorage, Join, Read, ReadStorage, System, Write};

pub struct Input {
    ibo: Ibo,
    vbo: Vbo,
    vao: Vao,
    pub data: Vec<f32>,
}

pub struct Mesh {
    pub inputs: Vec<Input>,
    indices: Vec<u32>,

    pub position: nalgebra_glm::Vec3,
    pub scale: nalgebra_glm::Vec3,
    // TODO: Rotation
}

impl Mesh {
    pub fn new(indices: Vec<u32>, datas: Vec<Vec<f32>>) -> Self {
        let inputs: Vec<Input> = datas
            .iter()
            .map(|data| Input {
                ibo: Ibo::gen(),
                vao: Vao::gen(),
                vbo: Vbo::gen(),
                data: data.to_vec(),
            })
            .collect();

        for i in 0..inputs.len() {
            inputs[i].vao.set(i as u32)
        }

        Mesh {
            inputs,
            indices,
            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            scale: nalgebra_glm::vec3(1.0, 1.0, 1.0),
        }
    }

    pub fn from_obj(obj_file_data: &[u8], color: nalgebra_glm::Vec3) -> Self {
        let obj: Obj<TexturedVertex> = load_obj(&obj_file_data[..]).unwrap();
        let vb: Vec<TexturedVertex> = obj.vertices;

        let indices = vec_u32_from_vec_u16(&obj.indices);
        let vertices = flatten_positions(&vb);
        let normals = flatten_normals(&vb);
        let uv = flatten_uv(&vb);
        let colors = (0..vertices.len() / 3)
            .flat_map(|_| {
                let repeat = vec![color.x, color.y, color.z];
                repeat.iter().cloned().collect::<Vec<_>>()
            })
            .collect();

        let data = vec![vertices, normals, uv, colors];

        Self::new(indices, data)
    }

    pub fn set_3d(program: &Program, sun_dir: nalgebra_glm::Vec3, resolution: nalgebra_glm::Vec2) {
        program.set();
        let u_resolution = Uniform::new(program.id(), "u_resolution").unwrap();
        let u_sun_dir = Uniform::new(program.id(), "u_sun_dir").unwrap();
        unsafe {
            gl::Uniform2f(u_resolution.id, resolution.x, resolution.y);
            gl::Uniform3f(u_sun_dir.id, sun_dir.x, sun_dir.y, sun_dir.z);
        }
    }

    pub fn get_model_matrix(
        position: nalgebra_glm::Vec3,
        scale: nalgebra_glm::Vec3,
    ) -> nalgebra_glm::Mat4 {
        let mut model_matrix = nalgebra_glm::one();
        model_matrix = nalgebra_glm::translate(&model_matrix, &position);
        model_matrix = nalgebra_glm::scale(&model_matrix, &scale);
        model_matrix
    }

    pub fn draw(
        &self,
        program: &Program,
        camera: &Camera,
        position: nalgebra_glm::Vec3,
        scale: nalgebra_glm::Vec3,
    ) {
        let u_model_matrix = Uniform::new(program.id(), "u_model_matrix").unwrap();
        let u_view_matrix = Uniform::new(program.id(), "u_view_matrix").unwrap();
        let u_proj_matrix = Uniform::new(program.id(), "u_proj_matrix").unwrap();
        let model_matrix = Mesh::get_model_matrix(position, scale);
        let (view_matrix, proj_matrix) = camera.gen_view_proj_matrices();
        unsafe {
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
            self.set();
            gl::DrawElements(
                gl::TRIANGLES,
                self.indices_len(),
                gl::UNSIGNED_INT,
                0 as *const _,
            );
        }
    }

    fn indices_len(&self) -> i32 {
        self.indices.len() as i32
    }

    fn set(&self) {
        for i in 0..self.inputs.len() {
            self.inputs[i].vbo.set(&self.inputs[i].data);
            self.inputs[i].vao.enable(i as u32);
            self.inputs[i].ibo.set(&self.indices);
        }
    }
}

fn flatten_positions(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.position[0]);
        retval.push(vertex.position[1]);
        retval.push(vertex.position[2]);
    }
    retval
}

fn flatten_normals(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.normal[0]);
        retval.push(vertex.normal[1]);
        retval.push(vertex.normal[2]);
    }
    retval
}

fn flatten_uv(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.texture[0]);
        retval.push(vertex.texture[1]);
        retval.push(vertex.texture[2]);
    }
    retval
}

fn vec_u32_from_vec_u16(input: &Vec<u16>) -> Vec<u32> {
    let mut retval = vec![];
    for x in input {
        retval.push(*x as u32);
    }
    retval
}

#[derive(Default)]
pub struct MeshMgr {
    meshes: Vec<Mesh>,
}

impl MeshMgr {
    pub fn new() -> Self {
        Self { meshes: vec![] }
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        let id = self.meshes.len();
        self.meshes.push(mesh);
        id
    }

    pub fn get_mesh(&self, id: usize) -> &Mesh {
        self.meshes.get(id).unwrap()
    }
}

#[derive(Default)]
pub struct MeshMgrResource {
    pub data: MeshMgr,
}

#[derive(Default)]
pub struct OpenGlResource {
    pub camera: Camera,
    pub program: Program,
}

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct MeshComponent {
    pub mesh_id: usize,
    pub scale: nalgebra_glm::Vec3,
    pub texture: Texture,
    pub render_dist: Option<f32>, //< When Some, only render when the position is this close to the camera
}

pub struct Render3dSystem;
impl<'a> System<'a> for Render3dSystem {
    type SystemData = (
        ReadStorage<'a, MeshComponent>,
        ReadStorage<'a, PositionComponent>,
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
            // Cull models that are too far away
            match renderable.render_dist {
                Some(d) => {
                    if nalgebra_glm::length(&(position.pos - open_gl.camera.position)) > d {
                        continue;
                    }
                }
                None => {}
            }
            // Cull models that are behind the player
            // (TODO: This is incredibly crude, and models that sorta "reach" into the viewport but whose position is behind the player are eroneously culled)
            // let view_ray = open_gl.camera.lookat - open_gl.camera.position;
            // let model_to_player_ray = position.pos - open_gl.camera.position;
            // if nalgebra_glm::dot(&view_ray, &model_to_player_ray) < 0.0 {
            //     continue;
            // }

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
