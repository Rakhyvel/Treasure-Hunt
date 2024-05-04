use super::objects::*;

use std::ffi::CString;
use std::path::Path;

use obj::{load_obj, Obj, TexturedVertex};

pub struct Input {
    ibo: Ibo,
    vbo: Vbo,
    vao: Vao,
    pub data: Vec<f32>,
}

pub struct Mesh {
    pub inputs: Vec<Input>,
    indices: Vec<u16>,
    texture: Texture,
}

impl Mesh {
    pub fn new(indices: Vec<u16>, datas: Vec<Vec<f32>>, texture: Texture) -> Self {
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
            texture,
            indices,
        }
    }

    pub fn from_obj(obj_file_data: &[u8], color: nalgebra_glm::Vec3, texture: Texture) -> Self {
        let obj: Obj<TexturedVertex> = load_obj(&obj_file_data[..]).unwrap();
        let vb: Vec<TexturedVertex> = obj.vertices;

        let indices = obj.indices;
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

        Self::new(indices, data, texture)
    }

    pub fn set(&self, program: u32) {
        self.texture.activate(gl::TEXTURE0);
        let uniform = CString::new("texture0").unwrap();
        unsafe { gl::Uniform1i(gl::GetUniformLocation(program, uniform.as_ptr()), 0) };

        for i in 0..self.inputs.len() {
            self.inputs[i].vbo.set(&self.inputs[i].data);
            self.inputs[i].vao.enable(i as u32);
            self.inputs[i].ibo.set(&vec_u32_from_vec_u16(&self.indices));
        }
    }

    pub fn indices_len(&self) -> i32 {
        self.indices.len() as i32
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
