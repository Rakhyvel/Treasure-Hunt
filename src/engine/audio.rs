use std::sync::{Arc, Mutex};

use sdl2::mixer::{self, Chunk};

enum SoundCommand {
    Play(String, i32),
    Quit,
}

pub struct AudioManager {
    sender: std::sync::mpsc::Sender<SoundCommand>,
}

impl AudioManager {
    pub fn new() -> Self {
        // Create a new channel for sending & receiving SoundCommand's
        let (sender, receiver) = std::sync::mpsc::channel();

        // Spawn a new thread to handle audio playback
        std::thread::spawn(|| {
            // Initialize SDL2_mixer library with support for OGG files
            sdl2::mixer::init(sdl2::mixer::InitFlag::OGG).unwrap();
            sdl2::mixer::open_audio(
                44_100,
                sdl2::mixer::AUDIO_S16LSB,
                sdl2::mixer::DEFAULT_CHANNELS,
                1_024,
            )
            .unwrap();
            sdl2::mixer::allocate_channels(4);

            // Create a thread-safe shared vector of 16 Chunks. `None` means they are not playing, `Some` means they are
            let chunks: Arc<Mutex<Vec<Option<Chunk>>>> =
                Arc::new(Mutex::new((0..16).map(|_| None).collect()));

            // Pend on commands from the receiver
            for command in receiver {
                AudioManager::clear_unused_channels(&chunks);
                match command {
                    SoundCommand::Play(file_path, volume) => {
                        let sound_file = mixer::Chunk::from_file(&file_path).unwrap();
                        // Lock the `channels` mutex to get exclusive access to the channels vector
                        let mut chunks = chunks.lock().unwrap();
                        // Find the first available (non-None) channel
                        if let Some((i, _)) = chunks
                            .iter_mut()
                            .enumerate()
                            .find(|(_, slot)| slot.is_none())
                        {
                            chunks[i] = Some(sound_file);
                            let channel = mixer::Channel(i as i32);
                            channel.set_volume(volume);
                            channel.play(chunks[i].as_ref().unwrap(), 0).unwrap();
                        } else {
                            println!("No available channel to play sound: {}", file_path);
                        }
                    }

                    SoundCommand::Quit => break,
                }
            }

            // Clean up SDL2_mixer
            sdl2::mixer::close_audio();
        });

        Self { sender }
    }

    fn clear_unused_channels(chunks: &Arc<Mutex<Vec<Option<Chunk>>>>) {
        let mut chunks = chunks.lock().unwrap();
        for i in 0..16 {
            if !mixer::Channel(i as i32).is_playing() {
                chunks[i] = None
            }
        }
    }

    /// Plays a sound.
    /// - file_path: relative to the crate directory
    /// - volume: [0, 128], anything above 128 is clipped to 128.
    pub fn play_sound(&self, file_path: String, volume: i32) {
        self.sender
            .send(SoundCommand::Play(file_path, volume))
            .unwrap();
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        println!("Audio manager dropped, btw!");
        self.sender.send(SoundCommand::Quit).unwrap();
    }
}

pub struct AudioResource {
    pub audio_mgr: AudioManager,
}
#[allow(unreachable_code)]
impl Default for AudioResource {
    fn default() -> Self {
        println!("default called, whuh oh!");
        Self { audio_mgr: todo!() }
    }
}
