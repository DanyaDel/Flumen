use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
use std::sync::Arc;
use flumen_engine::engine::AudioEngine;

mod gui_state;
mod renderer;

use egui_wgpu::wgpu;
use crate::gui_state::State;

struct App {
    audio_engine: AudioEngine,
    state: Option<State>,
}

impl App {
    fn new() -> Self {
        let mut audio_engine = AudioEngine::new();
        if let Err(e) = audio_engine.run() {
            eprintln!("Failed to start audio engine: {}", e);
        }
        Self {
            audio_engine,
            state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Flumen DAW")
                .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720));
            
            let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
            
            // Запускаем инициализацию State асинхронно
            // В Winit 0.30 нам нужно дождаться инициализации.
            // Для упрощения сделаем это блокирующим образом через block_on,
            // так как это происходит один раз при запуске.
            let state = pollster::block_on(State::new(window.clone()));
            self.state = Some(state);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(state) => state,
            None => return,
        };

        // Pass event to egui
        let window_arc = state.window_arc();
        if let WindowEvent::KeyboardInput { 
            event: winit::event::KeyEvent { 
                physical_key: winit::keyboard::PhysicalKey::Code(code), 
                state: element_state, 
                repeat: false,
                .. 
            }, 
            .. 
        } = event {
            // Musical Typing Scancodes
            use winit::keyboard::KeyCode;
            
            let midi_note = match code {
                // Lower Octave (C4 - E5)
                KeyCode::KeyZ => Some(60), // C4
                KeyCode::KeyS => Some(61), // C#4
                KeyCode::KeyX => Some(62), // D4
                KeyCode::KeyD => Some(63), // D#4
                KeyCode::KeyC => Some(64), // E4
                KeyCode::KeyV => Some(65), // F4
                KeyCode::KeyG => Some(66), // F#4
                KeyCode::KeyB => Some(67), // G4
                KeyCode::KeyH => Some(68), // G#4
                KeyCode::KeyN => Some(69), // A4
                KeyCode::KeyJ => Some(70), // A#4
                KeyCode::KeyM => Some(71), // B4
                KeyCode::Comma => Some(72), // C5
                KeyCode::KeyL => Some(73), // C#5
                KeyCode::Period => Some(74), // D5
                KeyCode::Semicolon => Some(75), // D#5
                KeyCode::Slash => Some(76), // E5

                // Upper Octave (C5 - E6)
                KeyCode::KeyQ => Some(72), // C5
                KeyCode::Digit2 => Some(73), // C#5
                KeyCode::KeyW => Some(74), // D5
                KeyCode::Digit3 => Some(75), // D#5
                KeyCode::KeyE => Some(76), // E5
                KeyCode::KeyR => Some(77), // F5
                KeyCode::Digit5 => Some(78), // F#5
                KeyCode::KeyT => Some(79), // G5
                KeyCode::Digit6 => Some(80), // G#5
                KeyCode::KeyY => Some(81), // A5
                KeyCode::Digit7 => Some(82), // A#5
                KeyCode::KeyU => Some(83), // B5
                KeyCode::KeyI => Some(84), // C6
                KeyCode::Digit9 => Some(85), // C#6
                KeyCode::KeyO => Some(86), // D6
                KeyCode::Digit0 => Some(87), // D#6
                KeyCode::KeyP => Some(88), // E6
                
                KeyCode::Space => {
                    if element_state == winit::event::ElementState::Pressed {
                        let mut graph = self.audio_engine.graph.lock().unwrap();
                        graph.engine.is_playing = !graph.engine.is_playing;
                    }
                    return; // Consumed
                }
                _ => None,
            };

            if let Some(midi) = midi_note {
                let mut graph = self.audio_engine.graph.lock().unwrap();
                let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
                if let Some(track_idx) = state.selected_track {
                    if let Some(track) = graph.engine.tracks.get_mut(track_idx) {
                        if element_state == winit::event::ElementState::Pressed {
                            track.node.trigger_freq(freq);
                        } else {
                            track.node.release_freq(freq);
                        }
                    }
                }
                return; // Consumed
            }
        }

        let response = state.egui_state.on_window_event(&window_arc, &event);
        if response.consumed {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            },
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size);
            },
            WindowEvent::MouseInput { .. } => {
                // Handled in gui_state.rs or ignored
            },
            WindowEvent::KeyboardInput { .. } => {
                // Handled above before egui
            },
            WindowEvent::RedrawRequested => {
                state.update(&self.audio_engine);
                match state.render(&self.audio_engine) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("{:?}", e),
                }
            },
            _ => (),
        }
        state.window().request_redraw();
    }
}

fn main() {
    env_logger::init();
    println!("Flumen DAW - Starting...");

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    
    event_loop.run_app(&mut app).unwrap();
}
