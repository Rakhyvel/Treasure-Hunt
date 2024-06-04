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
// - Health bar, health system
// - Actual treasure chest mesh
// - Gold icon for a treasure map that's successful
// - Menu, game-over, win, pause screens
// - Save serialization
// - Sound

fn main() -> Result<(), String> {
    run(800, 600, "Treasure Hunt", &|_app| {
        RefCell::new(Box::new(Island::new()))
    })
}
