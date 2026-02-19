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
            WindowEvent::KeyboardInput { 
                event: winit::event::KeyEvent { 
                    logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space), 
                    state: winit::event::ElementState::Pressed, 
                    .. 
                }, 
                .. 
            } => {
                let mut graph = self.audio_engine.graph.lock().unwrap();
                graph.engine.is_playing = !graph.engine.is_playing;
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
