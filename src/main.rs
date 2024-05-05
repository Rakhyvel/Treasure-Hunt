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
// - specs
// - Maps & Treasure
// - Enemy mobs
// - Health bar, weapons
// - Menu
// - Save serialization

fn main() -> Result<(), String> {
    run(800, 600, "Treasure Hunt", &|_app| {
        RefCell::new(Box::new(Island::new()))
    })
}
