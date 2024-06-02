pub enum ProjectionKind {
    Perspective {
        fov: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl Default for ProjectionKind {
    fn default() -> Self {
        Self::Perspective { fov: 3.5 }
    }
}

#[derive(Default)]
pub struct Camera {
    pub position: nalgebra_glm::Vec3,
    pub lookat: nalgebra_glm::Vec3,
    pub up: nalgebra_glm::Vec3,
    pub projection_kind: ProjectionKind,
}

impl Camera {
    pub fn new(
        position: nalgebra_glm::Vec3,
        lookat: nalgebra_glm::Vec3,
        up: nalgebra_glm::Vec3,
        projection_kind: ProjectionKind,
    ) -> Self {
        Self {
            position,
            lookat,
            up,
            projection_kind,
        }
    }

    pub fn gen_view_proj_matrices(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        let view_matrix = nalgebra_glm::look_at(&self.position, &self.lookat, &self.up);
        let proj_matrix = match self.projection_kind {
            ProjectionKind::Perspective { fov } => {
                nalgebra_glm::perspective(1.0, fov, 0.01, 9.296e+9)
            }
            ProjectionKind::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => nalgebra_glm::ortho(left, right, bottom, top, near, far),
        };
        (view_matrix, proj_matrix)
    }

    pub fn inv_proj_view(&self) -> nalgebra_glm::Mat4 {
        let (view, proj) = self.gen_view_proj_matrices();
        let proj_view = proj * view;
        nalgebra_glm::inverse(&proj_view)
    }
}
