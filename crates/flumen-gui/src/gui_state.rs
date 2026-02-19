use winit::window::Window;
use egui_wgpu::wgpu;
use std::fs;
use std::io;

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    window: std::sync::Arc<Window>,
    renderer: crate::renderer::Renderer,
    // egui fields
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
    // Edit state
    pub selected_track: Option<usize>,
    pub central_panel_rect: egui::Rect,
    pub is_piano_roll_open: bool,
    pub is_favus_open: bool,
    pub is_fluctus_open: bool,
    pub is_omnia_open: bool,
    // Piano Roll View state
    pub pr_scroll: egui::Vec2, // in steps/notes
    // Favus state
    pub favus_scroll: egui::Vec2,
    pub active_item_idx: Option<usize>, // item being dragged or resized
    pub is_resizing: bool,
}

impl State {
    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn window_arc(&self) -> std::sync::Arc<Window> {
        self.window.clone()
    }

    pub async fn new(window: std::sync::Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Используем Mailbox (Triple Buffering) если доступно, иначе FIFO (VSync)
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let renderer = crate::renderer::Renderer::new(&device, &config);

        // egui initialization
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            renderer,
            egui_ctx,
            egui_state,
            egui_renderer,
            selected_track: None,
            central_panel_rect: egui::Rect::NOTHING,
            is_piano_roll_open: false,
            is_favus_open: true,
            is_fluctus_open: false,
            is_omnia_open: false,
            pr_scroll: egui::Vec2::ZERO,
            favus_scroll: egui::Vec2::ZERO,
            active_item_idx: None,
            is_resizing: false,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn custom_knob(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
        ui.vertical(|ui| {
            ui.set_max_width(45.0);
            let knob_size = egui::vec2(40.0, 40.0);
            let (rect, response) = ui.allocate_exact_size(knob_size, egui::Sense::drag());

            if response.dragged() {
                let delta = response.drag_delta().y * -0.01;
                *value = (*value + delta).clamp(*range.start(), *range.end());
            }

            let visuals = ui.style().interact(&response);
            let painter = ui.painter();
            
            // Outer circle
            painter.circle_filled(rect.center(), rect.width() / 2.0, egui::Color32::from_rgb(35, 35, 40));
            painter.circle_stroke(rect.center(), rect.width() / 2.0, egui::Stroke::new(1.0, visuals.bg_fill));

            // Tick marks (simplified)
            let t = (*value - *range.start()) / (*range.end() - *range.start());
            let start_angle = std::f32::consts::PI * 0.75;
            let end_angle = std::f32::consts::PI * 2.25;
            let angle = start_angle + t * (end_angle - start_angle);

            // Pointer
            let pointer_len = rect.width() / 2.0 - 2.0;
            let pointer_pos = rect.center() + egui::vec2(angle.cos(), angle.sin()) * pointer_len;
            painter.line_segment([rect.center(), pointer_pos], egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 255, 150)));

            ui.add_space(2.0);
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(label).size(9.0).color(egui::Color32::GRAY));
            });
        });
    }

    fn waveform_selector(ui: &mut egui::Ui, current: &mut flumen_engine::graph::Waveform) {
        ui.horizontal(|ui| {
            let waves = [
                (flumen_engine::graph::Waveform::Sine, "∿"),
                (flumen_engine::graph::Waveform::Saw, "⊿"),
                (flumen_engine::graph::Waveform::Square, "⊓"),
                (flumen_engine::graph::Waveform::Triangle, "△"),
            ];
            for (w, icon) in waves {
                if ui.selectable_label(*current == w, icon).clicked() {
                    *current = w;
                }
            }
        });
    }

    fn save_to_file(project: &flumen_common::project::Project, path: &str) -> io::Result<()> {
        let json = serde_json::to_string_pretty(project)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, json)
    }

    fn load_from_file(path: &str) -> io::Result<flumen_common::project::Project> {
        let json = fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    pub fn update(&mut self, audio_engine: &flumen_engine::engine::AudioEngine) {
        let graph = audio_engine.graph.lock().unwrap();
        
        // --- 1. Update Grid Metrics for Renderer ---
        let rect = self.central_panel_rect;
        let zoom_x = 60.0;
        let zoom_y = 25.0;
        
        let piano_key_width_ratio = 0.06; 
        let grid_w = rect.width() * (1.0 - piano_key_width_ratio);
        let grid_h = rect.height();

        let visible_steps = grid_w / zoom_x;
        let visible_notes = grid_h / zoom_y;

        self.renderer.update_grid(
            &self.queue, 
            &graph.engine.tracks, 
            self.selected_track, 
            graph.engine.current_step,
            self.pr_scroll,
            visible_notes,
            visible_steps
        );
    }

    pub fn render(&mut self, audio_engine: &flumen_engine::engine::AudioEngine) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // --- 1. egui UI frame start ---
        let raw_input = self.egui_state.take_egui_input(&self.window);
        self.egui_ctx.begin_pass(raw_input);
        self.egui_ctx.request_repaint();

        let mut central_rect = egui::Rect::NOTHING;

        {
            let mut graph = audio_engine.graph.lock().unwrap();
            
            // --- 0. Top Menu Bar ---
            egui::TopBottomPanel::top("menu_bar").show(&self.egui_ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New Project").clicked() {
                            graph.engine.tracks.clear();
                            graph.engine.tracks.push(flumen_engine::graph::EngineTrack {
                                node: Box::new(flumen_engine::graph::MultiWaveSynth::new(440.0, flumen_engine::graph::Waveform::Sine)),
                                patterns: vec![flumen_engine::graph::Pattern::default()],
                                current_pattern_idx: 0,
                                volume: 0.7,
                                pan: 0.0,
                            });
                            ui.close_menu();
                        }
                        if ui.button("Open").clicked() {
                            match Self::load_from_file("project.flumen") {
                                Ok(proj) => graph.load_project(proj),
                                Err(e) => eprintln!("Load failed: {}", e),
                            }
                            ui.close_menu();
                        }
                        if ui.button("Save").clicked() {
                            let proj = graph.save_project("My Flumen Project".to_string());
                            if let Err(e) = Self::save_to_file(&proj, "project.flumen") {
                                eprintln!("Save failed: {}", e);
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Exit").clicked() { /* Exit handled by window close button for now */ ui.close_menu(); }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.button("Undo").clicked() { ui.close_menu(); }
                    });
                    ui.menu_button("View", |ui| {
                        ui.checkbox(&mut self.is_favus_open, "Favus (Arrangement)");
                        ui.checkbox(&mut self.is_piano_roll_open, "Piano Roll");
                        ui.checkbox(&mut self.is_omnia_open, "Omnia FX");
                        ui.checkbox(&mut self.is_fluctus_open, "Fluctus Synth");
                    });
                    ui.menu_button("Plugins", |ui| {
                        if ui.button("Fluctus (Synth)").clicked() { 
                            self.is_fluctus_open = true;
                            ui.close_menu(); 
                        }
                    });
                });
            });

            // --- 1. Transport Bar ---
            egui::TopBottomPanel::top("transport_top").show(&self.egui_ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.visuals_mut().button_frame = true;
                    
                    let play_text = if graph.engine.is_playing { "⏸ PAUSE" } else { "▶ PLAY" };
                    let btn_color = if graph.engine.is_playing { egui::Color32::from_rgb(180, 40, 40) } else { egui::Color32::from_rgb(40, 150, 40) };
                    
                    if ui.add_sized([80.0, 26.0], egui::Button::new(egui::RichText::new(play_text).strong()).fill(btn_color)).clicked() {
                        graph.engine.is_playing = !graph.engine.is_playing;
                    }

                    ui.separator();
                    
                    if ui.button("🎹 Piano Roll").clicked() {
                        self.is_piano_roll_open = !self.is_piano_roll_open;
                    }
                    if ui.button("🌀 Omnia FX").clicked() {
                        self.is_omnia_open = !self.is_omnia_open;
                    }
                    if ui.button("🔌 Fluctus Synth").clicked() {
                        if !self.is_fluctus_open && self.selected_track.is_none() {
                            self.selected_track = Some(0);
                        }
                        self.is_fluctus_open = !self.is_fluctus_open;
                    }

                    ui.separator();
                    ui.label(egui::RichText::new("BPM:").strong());
                    ui.add(egui::DragValue::new(&mut graph.engine.bpm).range(20.0..=300.0).speed(1.0));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("STEP: {}", graph.engine.current_step + 1));
                    });
                });
                ui.add_space(2.0);
            });

            // Mixer (Left)
            egui::SidePanel::left("mixer_left").resizable(false).exact_width(180.0).show(&self.egui_ctx, |ui| {
                ui.add_space(10.0);
                ui.heading("🎚 Mixer");
                ui.separator();
                ui.add_space(5.0);

                for (idx, track) in graph.engine.tracks.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.set_width(160.0);
                        ui.horizontal(|ui| {
                            let label = format!("TRK {:02}", idx + 1);
                            if ui.selectable_label(self.selected_track == Some(idx), egui::RichText::new(label).strong()).clicked() {
                                self.selected_track = if self.selected_track == Some(idx) { None } else { Some(idx) };
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{:.2}", track.volume)).size(10.0));
                            });
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            Self::custom_knob(ui, &mut track.volume, 0.0..=1.0, "VOL");
                            ui.add_space(15.0);
                            Self::custom_knob(ui, &mut track.pan, -1.0..=1.0, "PAN");
                        });
                    });
                    ui.add_space(4.0);
                }
            });

            // --- 2. Floating Windows ---
            
            // Omnia FX Window
            if self.is_omnia_open {
                let mut omnia_open = self.is_omnia_open;
                // graph is already locked in outer scope
                egui::Window::new("🌀 Omnia - Multi FX Master")
                    .open(&mut omnia_open)
                    .id(egui::Id::new("omnia_window_v1"))
                    .default_size([300.0, 400.0])
                    .resizable(true)
                    .show(&self.egui_ctx, |ui: &mut egui::Ui| {
                        ui.checkbox(&mut graph.omnia.is_enabled, "Enabled");
                        ui.separator();

                        ui.collapsing("🎸 Distortion", |ui: &mut egui::Ui| {
                            ui.add(egui::Slider::new(&mut graph.omnia.dist_mix, 0.0..=1.0).text("Mix"));
                            ui.add(egui::Slider::new(&mut graph.omnia.dist_drive, 0.0..=1.0).text("Drive"));
                        });

                        ui.collapsing("🎛️ Equalizer", |ui: &mut egui::Ui| {
                            ui.add(egui::Slider::new(&mut graph.omnia.eq_low, -1.0..=1.0).text("Low Shelf"));
                            ui.add(egui::Slider::new(&mut graph.omnia.eq_high, -1.0..=1.0).text("High Shelf"));
                        });

                        ui.collapsing("⏲️ Delay", |ui: &mut egui::Ui| {
                            ui.add(egui::Slider::new(&mut graph.omnia.delay_mix, 0.0..=1.0).text("Mix"));
                            ui.add(egui::Slider::new(&mut graph.omnia.delay_time, 0.05..=1.0).text("Time (s)"));
                            ui.add(egui::Slider::new(&mut graph.omnia.delay_feedback, 0.0..=0.9).text("Feedback"));
                        });

                        ui.collapsing("🌊 Reverb", |ui: &mut egui::Ui| {
                            ui.add(egui::Slider::new(&mut graph.omnia.reverb_mix, 0.0..=1.0).text("Mix"));
                            ui.add(egui::Slider::new(&mut graph.omnia.reverb_size, 0.0..=1.0).text("Size"));
                        });
                        
                        ui.add_space(10.0);
                        if ui.button("Reset All").clicked() {
                            graph.omnia.delay_mix = 0.0;
                            graph.omnia.reverb_mix = 0.0;
                            graph.omnia.dist_mix = 0.0;
                            graph.omnia.eq_low = 0.0;
                            graph.omnia.eq_high = 0.0;
                        }
                    });
                self.is_omnia_open = omnia_open;
            }

            // Fluctus Synth Window
            let mut fluctus_open = self.is_fluctus_open;
            egui::Window::new("🔌 Fluctus Synth - Lead")
                .open(&mut fluctus_open)
                .default_size([400.0, 500.0])
                .show(&self.egui_ctx, |ui| {
                    if self.selected_track.is_none() {
                        self.selected_track = Some(0);
                    }
                    
                    if let Some(idx) = self.selected_track {
                        if let Some(track) = graph.engine.tracks.get_mut(idx) {
                            if let Some(synth) = track.node.as_any_mut().downcast_mut::<flumen_engine::graph::MultiWaveSynth>() {
                                ui.heading(format!("Track {:02} Parameters", idx+1));
                                ui.add_space(10.0);

                                // Waveform Visualizer
                                ui.group(|ui| {
                                    ui.label("Waveform Visualizer:");
                                    let (rect, _response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 100.0), egui::Sense::hover());
                                    let painter = ui.painter();
                                    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 25));
                                    
                                    let color = egui::Color32::from_rgb(0, 255, 150);
                                    let points: Vec<egui::Pos2> = (0..100).map(|i| {
                                        let x = rect.min.x + (i as f32 / 100.0) * rect.width();
                                        let p = i as f32 / 100.0;
                                        let y_norm = match synth.waveform {
                                            flumen_engine::graph::Waveform::Sine => (p * std::f32::consts::TAU).sin(),
                                            flumen_engine::graph::Waveform::Saw => 2.0 * (p - 0.5),
                                            flumen_engine::graph::Waveform::Square => if p < 0.5 { 0.8 } else { -0.8 },
                                            flumen_engine::graph::Waveform::Triangle => if p < 0.25 { 4.0 * p } else if p < 0.75 { 2.0 - 4.0 * p } else { -4.0 + 4.0 * p },
                                        };
                                        let y = rect.center().y - y_norm * (rect.height() * 0.4);
                                        egui::pos2(x, y)
                                    }).collect();
                                    
                                    for window in points.windows(2) {
                                        painter.line_segment([window[0], window[1]], egui::Stroke::new(2.0, color));
                                    }
                                });
                                
                                ui.add_space(10.0);
                                
                                ui.group(|ui| {
                                    ui.label("Oscillator:");
                                    Self::waveform_selector(ui, &mut synth.waveform);
                                });
                                
                                ui.add_space(10.0);
                                ui.group(|ui| {
                                    ui.label("ADSR Envelope:");
                                    ui.horizontal_wrapped(|ui| {
                                        Self::custom_knob(ui, &mut synth.adsr.attack, 0.001..=1.0, "ATK");
                                        Self::custom_knob(ui, &mut synth.adsr.decay, 0.001..=1.0, "DEC");
                                        Self::custom_knob(ui, &mut synth.adsr.sustain, 0.0..=1.0, "SUS");
                                        Self::custom_knob(ui, &mut synth.adsr.release, 0.001..=2.0, "REL");
                                    });
                                });
                            }
                        }
                    }
                });
            self.is_fluctus_open = fluctus_open;

            // Main Background area / Piano Roll
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(&self.egui_ctx, |ui| {
                    let total_available = ui.available_size();
                    let show_favus = self.is_favus_open;
                    let show_piano = self.is_piano_roll_open;

                    let (favus_h, piano_h) = match (show_favus, show_piano) {
                        (true, true) => (total_available.y * 0.4, total_available.y * 0.6),
                        (true, false) => (total_available.y, 0.0),
                        (false, true) => (0.0, total_available.y),
                        (false, false) => (0.0, 0.0),
                    };

                    if show_favus {
                        ui.allocate_ui(egui::vec2(total_available.x, favus_h), |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.heading("🏁 Favus Arrangement");
                                    ui.add_space(20.0);
                                    if ui.button("Clear Playlist").clicked() {
                                        graph.engine.playlist.clear();
                                    }
                                });
                                ui.separator();

                                let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                                let painter = ui.painter_at(rect);
                                // Background
                                painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(25, 25, 30));

                                let track_h = 40.0;
                                let step_w = 10.0; // zoom_x for favus is different or fixed
                                let favus_zoom_x = 10.0;

                                // --- Interaction Logic for Favus ---
                                if response.hovered() {
                                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                                    self.favus_scroll.x -= scroll_delta.x / favus_zoom_x;
                                    self.favus_scroll.y -= scroll_delta.y / track_h;
                                    
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let rel_x = pos.x - rect.min.x;
                                        let rel_y = pos.y - rect.min.y;
                                        let step_idx = (rel_x / favus_zoom_x + self.favus_scroll.x) as u32;
                                        let track_idx = (rel_y / track_h + self.favus_scroll.y) as usize;

                                        if ui.input(|i| i.pointer.primary_clicked()) {
                                            // Place new pattern if not clicking an existing one
                                            let mut hit = false;
                                            for (i, item) in graph.engine.playlist.iter().enumerate() {
                                                let item_x = (item.start_step as f32 - self.favus_scroll.x) * favus_zoom_x + rect.min.x;
                                                let item_w = item.length as f32 * favus_zoom_x;
                                                let item_y = (item.track_id as f32 - self.favus_scroll.y) * track_h + rect.min.y;
                                                let item_rect = egui::Rect::from_min_size(egui::pos2(item_x, item_y), egui::vec2(item_w, track_h));
                                                
                                                if item_rect.contains(pos) {
                                                    self.active_item_idx = Some(i);
                                                    // Resize check (last 10 pixels)
                                                    self.is_resizing = pos.x > item_rect.max.x - 10.0;
                                                    hit = true;
                                                    break;
                                                }
                                            }

                                            if !hit {
                                                if let Some(t_idx) = self.selected_track {
                                                    graph.engine.playlist.push(flumen_engine::graph::ArrangementItem {
                                                        track_id: t_idx as u32,
                                                        pattern_index: 0,
                                                        start_step: step_idx,
                                                        length: 16,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }

                                if ui.input(|i| i.pointer.primary_released()) {
                                    self.active_item_idx = None;
                                    self.is_resizing = false;
                                }

                                if let Some(idx) = self.active_item_idx {
                                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                        if let Some(item) = graph.engine.playlist.get_mut(idx) {
                                            let rel_x = pos.x - rect.min.x;
                                            if self.is_resizing {
                                                let new_end_step = (rel_x / favus_zoom_x + self.favus_scroll.x) as u32;
                                                if new_end_step > item.start_step {
                                                    item.length = new_end_step - item.start_step;
                                                }
                                            } else {
                                                // Dragging (simplified: just moves start_step)
                                                item.start_step = (rel_x / favus_zoom_x + self.favus_scroll.x).max(0.0) as u32;
                                                let rel_y = pos.y - rect.min.y;
                                                item.track_id = (rel_y / track_h + self.favus_scroll.y).max(0.0) as u32;
                                            }
                                        }
                                    }
                                }

                                // --- Drawing Favus Grid ---
                                for i in 0..8 { // tracks
                                    let y = rect.min.y + (i as f32 - self.favus_scroll.y) * track_h;
                                    if y >= rect.min.y && y < rect.max.y {
                                        painter.line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)], (1.0, egui::Color32::from_gray(60)));
                                        painter.text(egui::pos2(rect.min.x + 5.0, y + 5.0), egui::Align2::LEFT_TOP, format!("Track {}", i+1), egui::FontId::proportional(12.0), egui::Color32::GRAY);
                                    }
                                }

                                for item in &graph.engine.playlist {
                                    let x = (item.start_step as f32 - self.favus_scroll.x) * favus_zoom_x + rect.min.x;
                                    let w = item.length as f32 * favus_zoom_x;
                                    let y = (item.track_id as f32 - self.favus_scroll.y) * track_h + rect.min.y;
                                    
                                    if x + w > rect.min.x && x < rect.max.x && y + track_h > rect.min.y && y < rect.max.y {
                                        let item_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, track_h)).shrink(2.0);
                                        painter.rect_filled(item_rect, 4.0, egui::Color32::from_rgb(100, 150, 255));
                                        painter.rect_stroke(item_rect, 4.0, (1.0, egui::Color32::WHITE.gamma_multiply(0.5)), egui::StrokeKind::Inside);
                                        painter.text(item_rect.center(), egui::Align2::CENTER_CENTER, format!("P{}", item.pattern_index), egui::FontId::proportional(14.0), egui::Color32::BLACK);
                                    }
                                }
                            });
                        });
                    }

                    if show_piano {
                        ui.allocate_ui(egui::vec2(total_available.x, piano_h), |ui| {
                            // --- Piano Roll View (Constrained to CentralPanel) ---
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                            let available = ui.available_size();
                            let scroll_v_w = 16.0;
                            let scroll_h_h = 24.0;
                            let grid_w = available.x - scroll_v_w;
                            let grid_h = available.y - scroll_h_h;

                            ui.horizontal(|ui| {
                                // GRID AREA
                                let (rect, response) = ui.allocate_exact_size(egui::vec2(grid_w, grid_h), egui::Sense::click_and_drag());
                                self.central_panel_rect = rect;
                                central_rect = rect;

                                let zoom_x = 60.0;
                                let zoom_y = 25.0;

                                // --- Interaction Logic (FIXED: Better egui handling to prevent leakage) ---
                                if response.hovered() {
                                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                                    if ui.input(|i| i.modifiers.shift) {
                                        self.pr_scroll.x -= scroll_delta.y / zoom_x;
                                    } else {
                                        self.pr_scroll.y -= scroll_delta.y / zoom_y;
                                    }
                                    self.pr_scroll.x -= scroll_delta.x / zoom_x;

                                    if response.dragged() {
                                        let delta = response.drag_delta();
                                        self.pr_scroll.x -= delta.x / zoom_x;
                                        self.pr_scroll.y -= delta.y / zoom_y;
                                    }

                                    // Note interaction (Primary button only if clicked on THIS response)
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let piano_key_width_ratio = 0.06;
                                        let grid_x_start = rect.min.x + rect.width() * piano_key_width_ratio;
                                        if pos.x > grid_x_start {
                                            let x_idx = ((pos.x - grid_x_start) / zoom_x + self.pr_scroll.x) as i32;
                                            let y_idx = ((pos.y - rect.min.y) / zoom_y + self.pr_scroll.y) as i32;
                                            let pitch_idx = (120 - 1 - y_idx) as i32;

                                            if x_idx >= 0 && x_idx < 128 && pitch_idx >= 0 && pitch_idx < 120 {
                                                if let Some(track_idx) = self.selected_track {
                                                    if let Some(track) = graph.engine.tracks.get_mut(track_idx) {
                                                        if ui.input(|i| i.pointer.primary_clicked()) && response.clicked() {
                                                            track.current_pattern_mut().grid[pitch_idx as usize][x_idx as usize] = 1;
                                                        } else if ui.input(|i| i.pointer.secondary_clicked()) && response.secondary_clicked() {
                                                            track.current_pattern_mut().grid[pitch_idx as usize][x_idx as usize] = 0;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Note names (piano keys background)
                                let painter = ui.painter_at(rect);
                                for pitch in 0..120 {
                                    let row_in_view = (120 - 1 - pitch) as f32 - self.pr_scroll.y;
                                    let y_start = rect.min.y + row_in_view * zoom_y;
                                    if y_start > rect.min.y - zoom_y && y_start < rect.max.y {
                                        let midi = 12 + pitch;
                                        let note_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                                        let name = format!("{}{}", note_names[midi % 12], midi / 12 - 1);
                                        painter.text(
                                            egui::pos2(rect.min.x + 3.0, y_start + zoom_y/2.0),
                                            egui::Align2::LEFT_CENTER,
                                            name,
                                            egui::FontId::proportional(zoom_y.min(11.0).max(7.0)),
                                            if [1, 3, 6, 8, 10].contains(&(midi % 12)) { egui::Color32::GRAY.gamma_multiply(0.4) } else { egui::Color32::WHITE.gamma_multiply(0.2) }
                                        );
                                    }
                                }

                                // Vertical Slider
                                ui.spacing_mut().slider_width = grid_h;
                                let mut pr_scroll_inv_y = 120.0 - self.pr_scroll.y;
                                ui.add(egui::Slider::new(&mut pr_scroll_inv_y, 0.0..=120.0).vertical().show_value(false));
                                self.pr_scroll.y = 120.0 - pr_scroll_inv_y;
                            });

                            ui.horizontal(|ui| {
                                ui.add_space(grid_w * 0.06); // piano keys offset
                                ui.spacing_mut().slider_width = grid_w * 0.94;
                                let mut pr_scroll_x = self.pr_scroll.x;
                                ui.add(egui::Slider::new(&mut pr_scroll_x, 0.0..=128.0).show_value(false));
                                self.pr_scroll.x = pr_scroll_x;
                            });

                            // Clamping
                            let vis_n = grid_h / 25.0;
                            let vis_s = grid_w / 60.0;
                            self.pr_scroll.x = self.pr_scroll.x.clamp(0.0, (128.0_f32 - vis_s).max(0.0_f32));
                            self.pr_scroll.y = self.pr_scroll.y.clamp(0.0, (120.0_f32 - vis_n).max(0.0_f32));
                        });
                    }
                });
        }

        // --- 2. egui frame end and tessellate ---
        let full_output = self.egui_ctx.end_pass();
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor);

        // --- 3. WGPU Rendering Pass ---
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.08, g: 0.08, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // We need to use unsafe here because egui_wgpu's render expects a RenderPass with a specific lifetime
            // that is often hard to satisfy in a single function without transmute in some versions.
            unsafe {
                let mut rp: wgpu::RenderPass<'static> = std::mem::transmute(render_pass);
                
                // 1. Render WGPU Grid (Behind Egui)
                // Align WGPU viewport to Piano Roll window
                if self.is_piano_roll_open && central_rect.width() > 1.0 && central_rect.height() > 1.0 {
                    let scale = self.window.scale_factor() as f32;
                    let screen_w = self.config.width as f32;
                    let screen_h = self.config.height as f32;

                    let mut x = central_rect.min.x * scale;
                    let mut y = central_rect.min.y * scale;
                    let mut w = central_rect.width() * scale;
                    let mut h = central_rect.height() * scale;

                    x = x.clamp(0.0, screen_w);
                    y = y.clamp(0.0, screen_h);
                    w = w.min(screen_w - x).max(0.0);
                    h = h.min(screen_h - y).max(0.0);

                    if w > 1.0 && h > 1.0 {
                        rp.set_viewport(x, y, w, h, 0.0, 1.0);
                        self.renderer.render(&mut rp);
                    }
                }

                // 2. Render Egui (On Top)
                // Reset viewport to full screen for egui
                rp.set_viewport(0.0, 0.0, self.config.width as f32, self.config.height as f32, 0.0, 1.0);
                self.egui_renderer.render(&mut rp, &paint_jobs, &screen_descriptor);
            }
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
