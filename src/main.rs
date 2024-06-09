mod engine;
mod scenes;

use std::cell::RefCell;

use engine::app::*;
use scenes::island::Island;

// TODO:
// x Island generation
// x Camera movement
// x Trees
// x Text
// x ecs
// x Maps & Treasure
// x Enemy mobs
// x Health bar, health system, splish animation
// x Cylindrical collision with mobs, players, and trees etc
// x Actual treasure chest mesh
// x Gold icon for a treasure map that's successful, open treasure chest
// - Text that at least tells you what to do
// - Sound

fn main() -> Result<(), String> {
    run(800, 600, "Treasure Hunt", &|_app| {
        RefCell::new(Box::new(Island::new()))
    })
}
