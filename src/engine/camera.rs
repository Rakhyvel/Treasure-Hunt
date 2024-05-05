pub trait Camera {
    fn gen_view_proj_matrices(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4);
}

pub struct PerspectiveCamera {
    pub position: nalgebra_glm::Vec3,
    pub lookat: nalgebra_glm::Vec3,
    pub up: nalgebra_glm::Vec3,
    pub fov: f32,
}

impl PerspectiveCamera {
    pub fn new(
        position: nalgebra_glm::Vec3,
        lookat: nalgebra_glm::Vec3,
        up: nalgebra_glm::Vec3,
        fov: f32,
    ) -> Self {
        Self {
            position,
            lookat,
            up,
            fov,
        }
    }
}

impl Camera for PerspectiveCamera {
    fn gen_view_proj_matrices(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        let view_matrix = nalgebra_glm::look_at(&self.position, &self.lookat, &self.up);
        let proj_matrix = nalgebra_glm::perspective(1.0, self.fov, 0.01, 9.296e+9);
        (view_matrix, proj_matrix)
    }
}

pub struct OrthoCamera {
    pub position: nalgebra_glm::Vec3,
    pub lookat: nalgebra_glm::Vec3,
    pub up: nalgebra_glm::Vec3,
}

impl OrthoCamera {
    pub fn new(
        position: nalgebra_glm::Vec3,
        lookat: nalgebra_glm::Vec3,
        up: nalgebra_glm::Vec3,
    ) -> Self {
        Self {
            position,
            lookat,
            up,
        }
    }
}

impl Camera for OrthoCamera {
    fn gen_view_proj_matrices(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        let view_matrix = nalgebra_glm::look_at(&self.position, &self.lookat, &self.up);
        let proj_matrix = nalgebra_glm::ortho(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0);
        (view_matrix, proj_matrix)
    }
}
