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

    pub fn compute_frustum_corners(&self) -> [nalgebra_glm::Vec4; 8] {
        let inv_proj_view = self.inv_proj_view();
        let mut frustum_corners = [nalgebra_glm::Vec4::zeros(); 8];

        let (near, far) = (0.5, 0.999);

        let ndc_corners = [
            nalgebra_glm::vec4(-1.0, -1.0, near, 1.0), // near bottom left
            nalgebra_glm::vec4(1.0, -1.0, near, 1.0),  // near bottom right
            nalgebra_glm::vec4(1.0, 1.0, near, 1.0),   // near top right
            nalgebra_glm::vec4(-1.0, 1.0, near, 1.0),  // near top left
            //
            nalgebra_glm::vec4(-1.0, -1.0, far, 1.0), // far bottom left
            nalgebra_glm::vec4(1.0, -1.0, far, 1.0),  // far bottom right
            nalgebra_glm::vec4(1.0, 1.0, far, 1.0),   // far top right
            nalgebra_glm::vec4(-1.0, 1.0, far, 1.0),  // far top left
        ];

        for (i, &ndc_corner) in ndc_corners.iter().enumerate() {
            let clip_space_corner = inv_proj_view * ndc_corner;
            // Handle the case where w is 0 to avoid NaNs
            frustum_corners[i] = clip_space_corner / clip_space_corner.w;
        }

        frustum_corners
    }
}
