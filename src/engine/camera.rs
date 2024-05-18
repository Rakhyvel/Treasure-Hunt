pub enum ProjectionKind {
    Perspective { fov: f32 },
    Orthographic,
}

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
            ProjectionKind::Orthographic => nalgebra_glm::ortho(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0),
        };
        (view_matrix, proj_matrix)
    }
}
