#[derive(Debug)]
pub struct AABB {
    pub min: nalgebra_glm::Vec3,
    pub max: nalgebra_glm::Vec3,
}

impl AABB {
    pub fn new() -> Self {
        Self {
            min: nalgebra_glm::vec3(f32::MAX, f32::MAX, f32::MAX),
            max: nalgebra_glm::vec3(f32::MIN, f32::MIN, f32::MIN),
        }
    }

    pub fn from_min_max(min: nalgebra_glm::Vec3, max: nalgebra_glm::Vec3) -> Self {
        Self { min, max }
    }

    pub fn translate(&self, center: nalgebra_glm::Vec3) -> Self {
        Self {
            min: self.min + center,
            max: self.max + center,
        }
    }

    pub fn expand_to_fit(&mut self, points: impl IntoIterator<Item = nalgebra_glm::Vec3>) {
        for corner in points.into_iter() {
            self.min = nalgebra_glm::min2(&self.min, &corner.xyz());
            self.max = nalgebra_glm::max2(&self.max, &corner.xyz());
        }
    }

    pub fn pos_z_plane_midpoint(&self) -> nalgebra_glm::Vec4 {
        let bottom_left = nalgebra_glm::vec4(self.min.x, self.min.y, self.max.z, 1.0);
        let top_right = nalgebra_glm::vec4(self.max.x, self.max.y, self.max.z, 1.0);
        0.5 * (bottom_left + top_right)
    }

    pub fn transform(&mut self, matrix: nalgebra_glm::Mat4) {
        self.min = (matrix * nalgebra_glm::vec4(self.min.x, self.min.y, self.min.z, 1.0)).xyz();
        self.max = (matrix * nalgebra_glm::vec4(self.max.x, self.max.y, self.max.z, 1.0)).xyz();
    }

    pub fn intersect_z(&mut self, other: &AABB) {
        self.min.z = self.min.z.min(other.min.z);
        self.max.z = self.max.z.max(other.max.z);
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        // Check for separation in the x-axis
        if self.max.x < other.min.x || self.min.x > other.max.x {
            return false;
        }
        // Check for separation in the y-axis
        if self.max.y < other.min.y || self.min.y > other.max.y {
            return false;
        }
        // Check for separation in the z-axis
        if self.max.z < other.min.z || self.min.z > other.max.z {
            return false;
        }

        // No separation found, the AABBs intersect
        true
    }
}
