use std::cell::RefCell;
use std::time::Instant;

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Scancode;
use sdl2::sys::{SDL_GetPerformanceCounter, SDL_GetPerformanceFrequency};
use sdl2::video::SwapInterval;
use sdl2::Sdl;

#[derive(Clone)]
pub struct App {
    // Screen stuff
    pub screen_width: i32,
    pub screen_height: i32,
    // pub sdl_context: Sdl,

    // Main loop stuff
    pub running: bool,
    pub seconds: f32, //< How many seconds the program has been up

    // User input state
    pub keys: [bool; 256],
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_rel_x: i32,
    pub mouse_rel_y: i32,
    pub mouse_left_down: bool,
    pub mouse_right_down: bool,
    pub mouse_wheel: f32,
    // Scene stack stuff
}

pub fn run(
    screen_width: i32,
    screen_height: i32,
    window_title: &'static str,
    init: &dyn Fn(&App) -> RefCell<Box<dyn Scene>>,
) -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(3, 3);

    let window = video_subsystem
        .window(window_title, screen_width as u32, screen_height as u32)
        .resizable()
        .opengl()
        .build()
        .unwrap();

    let _gl_context = window.gl_create_context().unwrap();

    let _gl =
        gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    window
        .subsystem()
        .gl_set_swap_interval(SwapInterval::VSync)
        .unwrap();

    unsafe {
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LESS);
        gl::Enable(gl::CULL_FACE);
        gl::Enable(gl::MULTISAMPLE);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }

    let mut app = App {
        screen_width,
        screen_height,
        // sdl_context,
        running: true,
        keys: [false; 256],
        mouse_x: 0,
        mouse_y: 0,
        mouse_rel_x: 0,
        mouse_rel_y: 0,
        mouse_left_down: false,
        mouse_right_down: false,
        mouse_wheel: 0.0,
        seconds: 0.0,
        // scene_stack: Vec::new(),
    };

    let initial_scene = init(&app);
    let mut scene_stack: Vec<RefCell<Box<dyn Scene>>> = vec![];
    scene_stack.push(initial_scene);

    let time = Instant::now();
    let mut start = time.elapsed().as_millis();
    let mut current;
    let mut previous = 0;
    let mut lag = 0;
    let mut elapsed;
    let mut ticks = 0;
    const DELTA_T: u128 = 16;
    while app.running {
        app.seconds = time.elapsed().as_secs_f32();
        current = time.elapsed().as_millis();
        elapsed = current - previous;

        previous = current;
        lag += elapsed;

        let scene_stale = false;
        while lag >= DELTA_T {
            app.reset_input();
            app.poll_input(&sdl_context);
            sdl_context.mouse().warp_mouse_in_window(
                &window,
                app.screen_width / 2,
                app.screen_height / 2,
            );
            sdl_context.mouse().set_relative_mouse_mode(true);

            if let Some(scene_ref) = scene_stack.last() {
                scene_ref.borrow_mut().update(&app);
                ticks += 1;
            }

            if !scene_stale {
                // if scene isn't stale, purge the scene
                lag -= DELTA_T;
            } else {
                break;
            }
        }

        if !scene_stale {
            unsafe {
                gl::Viewport(0, 0, app.screen_width, app.screen_height);
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            }
            if let Some(scene_ref) = scene_stack.last() {
                scene_ref.borrow_mut().render(&app);
            }
            window.gl_swap_window();
        }

        let end = unsafe { SDL_GetPerformanceCounter() };
        let freq = unsafe { SDL_GetPerformanceFrequency() };
        let seconds = (end as f64 - (start as f64)) / (freq as f64);
        if seconds > 5.0 {
            println!("5 seconds; ticks: {}", ticks);
            start = end as u128;
            ticks = 0;
        }
    }
    Ok(())
}

impl App {
    fn reset_input(&mut self) {
        self.mouse_rel_x = 0;
        self.mouse_rel_y = 0;
        self.mouse_wheel = 0.0;
    }

    fn poll_input(&mut self, sdl_context: &Sdl) {
        let mut event_queue = sdl_context.event_pump().unwrap();
        for event in event_queue.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.running = false;
                }

                Event::MouseMotion {
                    x, y, xrel, yrel, ..
                } => {
                    self.mouse_x = x;
                    self.mouse_y = y;
                    self.mouse_rel_x = xrel;
                    self.mouse_rel_y = yrel;
                }

                Event::MouseButtonDown { mouse_btn, .. } => match mouse_btn {
                    sdl2::mouse::MouseButton::Left => self.mouse_left_down = true,
                    sdl2::mouse::MouseButton::Right => self.mouse_right_down = true,
                    _ => {}
                },

                Event::MouseButtonUp { mouse_btn, .. } => match mouse_btn {
                    sdl2::mouse::MouseButton::Left => self.mouse_left_down = false,
                    sdl2::mouse::MouseButton::Right => self.mouse_right_down = false,
                    _ => {}
                },

                Event::MouseWheel { y, .. } => {
                    self.mouse_wheel = y as f32;
                }

                Event::Window { win_event, .. } => {
                    if let WindowEvent::Resized(new_width, new_height) = win_event {
                        self.screen_width = new_width;
                        self.screen_height = new_height;
                    }
                }

                Event::KeyDown { scancode, .. } => match scancode {
                    Some(sc) => {
                        self.keys[sc as usize] = true;
                        if self.keys[Scancode::Escape as usize] {
                            self.running = false
                        }
                    }
                    None => {}
                },

                Event::KeyUp { scancode, .. } => match scancode {
                    Some(sc) => self.keys[sc as usize] = false,
                    None => {}
                },

                _ => {}
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen_width: Default::default(),
            screen_height: Default::default(),
            running: Default::default(),
            seconds: Default::default(),
            keys: [false; 256],
            mouse_x: Default::default(),
            mouse_y: Default::default(),
            mouse_rel_x: Default::default(),
            mouse_rel_y: Default::default(),
            mouse_left_down: Default::default(),
            mouse_right_down: Default::default(),
            mouse_wheel: Default::default(),
            // scene_stack: Default::default(),
        }
    }
}

pub trait Scene {
    // TODO: Return a "command" enum so that scene's can affect App state
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}
