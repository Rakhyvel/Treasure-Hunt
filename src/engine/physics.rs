use specs::{Component, DenseVecStorage};

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct PositionComponent {
    pub pos: nalgebra_glm::Vec3,
}

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct VelocityComponent {
    pub vel: nalgebra_glm::Vec3,
}
