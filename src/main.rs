mod engine;
mod scenes;

use std::cell::RefCell;

use engine::app::*;
use scenes::island::Island;

fn main() -> Result<(), String> {
    run(800, 600, "Treasure Hunt", &|_app| {
        RefCell::new(Box::new(Island::new()))
    })
}
