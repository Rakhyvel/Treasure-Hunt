#[derive(Clone)]
pub struct Frustrum {
    pub points: [nalgebra_glm::Vec3; 8],
}

impl Frustrum {
    pub fn new(near: f32, far: f32) -> Self {
        Self {
            points: [
                nalgebra_glm::vec3(-2.0, -1.0, near), // near bottom left
                nalgebra_glm::vec3(2.0, -1.0, near),  // near bottom right
                nalgebra_glm::vec3(2.0, 1.0, near),   // near top right
                nalgebra_glm::vec3(-2.0, 1.0, near),  // near top left
                //
                nalgebra_glm::vec3(-2.0, -1.0, far), // far bottom left
                nalgebra_glm::vec3(2.0, -1.0, far),  // far bottom right
                nalgebra_glm::vec3(2.0, 1.0, far),   // far top right
                nalgebra_glm::vec3(-2.0, 1.0, far),  // far top left
            ],
        }
    }

    pub fn transform_points(&mut self, matrix: nalgebra_glm::Mat4) {
        let mut temp = self.points;
        for (i, &ndc_corner) in self.points.iter().enumerate() {
            let ndc_corner = nalgebra_glm::vec4(ndc_corner.x, ndc_corner.y, ndc_corner.z, 1.0);
            let clip_space_corner = matrix * ndc_corner;
            temp[i] = (clip_space_corner / clip_space_corner.w).xyz();
        }
        self.points = temp;
    }
}
