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
    pub pr_selection: Option<(egui::Vec2, egui::Vec2)>, // start and end of selection
    pub pr_selected_notes: Vec<(usize, usize)>, // selected notes (pitch, step)
    pub pr_draw_mode: Option<bool>, // true = draw, false = erase
    pub pr_note_length: u8, // default note length in steps
    pub pr_last_clicked_note: Option<(usize, usize)>, // for drag detection
    pub pr_is_dragging: bool,
    pub pr_hover_note: Option<(usize, usize, f32)>, // pitch, step, offset_in_note
    pub pr_resize_mode: bool,
    // Favus state
    pub favus_scroll: egui::Vec2,
    pub active_item_idx: Option<usize>, // item being dragged or resized
    pub is_resizing: bool,
    pub mouse_played_note: Option<f32>,
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
            pr_selection: None,
            pr_selected_notes: Vec::new(),
            pr_draw_mode: None,
            pr_note_length: 4,
            pr_last_clicked_note: None,
            pr_is_dragging: false,
            pr_hover_note: None,
            pr_resize_mode: false,
            favus_scroll: egui::Vec2::ZERO,
            active_item_idx: None,
            is_resizing: false,
            mouse_played_note: None,
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

    fn custom_knob(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) -> bool {
        let mut changed = false;
        ui.vertical(|ui| {
            ui.set_max_width(50.0);
            let knob_size = egui::vec2(38.0, 38.0);
            let id = ui.make_persistent_id(label);
            let mut smooth_val = ui.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || *value));

            let (rect, response) = ui.allocate_exact_size(knob_size, egui::Sense::click_and_drag());

            if response.dragged() {
                let delta = response.drag_delta().y * -0.005;
                smooth_val = (smooth_val + delta).clamp(*range.start(), *range.end());
                ui.data_mut(|d| d.insert_temp(id, smooth_val));
                *value = smooth_val;
                changed = true;
            } else if !response.hovered() && !response.dragged() {
                ui.data_mut(|d| d.insert_temp(id, *value));
            }

            let painter = ui.painter();
            let circle_color = if response.hovered() || response.dragged() { egui::Color32::from_rgb(60, 60, 70) } else { egui::Color32::from_rgb(40, 40, 45) };
            
            painter.circle_filled(rect.center(), rect.width() / 2.0, circle_color);
            painter.circle_stroke(rect.center(), rect.width() / 2.0, egui::Stroke::new(1.0, egui::Color32::BLACK));

            let t = (*value - *range.start()) / (*range.end() - *range.start());
            let start_angle = 0.75 * std::f32::consts::PI;
            let end_angle = 2.25 * std::f32::consts::PI;
            let angle = start_angle + t * (end_angle - start_angle);

            let radius = rect.width() / 2.0 - 3.0;
            let mut points = Vec::new();
            for i in 0..=30 {
                let a = start_angle + (i as f32 / 30.0) * (angle - start_angle);
                points.push(rect.center() + egui::vec2(a.cos(), a.sin()) * radius);
            }
            painter.add(egui::Shape::line(points, egui::Stroke::new(3.0, egui::Color32::from_rgb(0, 200, 255))));

            let p_pos = rect.center() + egui::vec2(angle.cos(), angle.sin()) * radius;
            painter.line_segment([rect.center(), p_pos], egui::Stroke::new(1.5, egui::Color32::WHITE));

            ui.add_space(2.0);
            ui.label(egui::RichText::new(label).size(9.0).color(egui::Color32::from_gray(180)).strong());
        });
        changed
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
                let is_sel = *current == w;
                let btn = egui::Button::new(egui::RichText::new(icon).size(14.0))
                    .frame(false)
                    .fill(if is_sel { egui::Color32::from_rgba_unmultiplied(0, 200, 255, 100) } else { egui::Color32::TRANSPARENT });
                if ui.add(btn).clicked() {
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
        
        // --- Premium Styling ---
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(15, 15, 18);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(20, 20, 25);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 35);
        visuals.selection.bg_fill = egui::Color32::from_rgb(0, 150, 255);
        self.egui_ctx.set_visuals(visuals);

        self.egui_ctx.request_repaint();

        let mut central_rect = egui::Rect::NOTHING;

        {
            let mut graph = audio_engine.graph.lock().unwrap();
            
            // --- TOP PANEL: Main Toolbar ---
            egui::TopBottomPanel::top("main_toolbar").exact_height(35.0).show(&self.egui_ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // 1. Menu Section
                    ui.menu_button(egui::RichText::new("☰ FLUMEN").strong().color(egui::Color32::from_rgb(0, 200, 255)), |ui| {
                        if ui.button("New Project").clicked() {
                            graph.engine.tracks.clear();
                            graph.engine.tracks.push(flumen_engine::graph::EngineTrack {
                                node: Box::new(flumen_engine::graph::PolySynth::new(flumen_engine::graph::Waveform::Sine)),
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
                        if ui.button("📤 Export to WAV (30s)").clicked() {
                            if let Err(e) = audio_engine.export_to_wav("export.wav", 30.0) {
                                eprintln!("Export failed: {}", e);
                            } else {
                                println!("Export completed: export.wav");
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Exit").clicked() { ui.close_menu(); }
                    });
                    ui.separator();

                    // 2. View Toggles
                    let tg_color = |on| if on { egui::Color32::from_rgb(200, 255, 200) } else { egui::Color32::GRAY };
                    if ui.selectable_label(self.is_favus_open, egui::RichText::new("🏁 Arr").color(tg_color(self.is_favus_open))).clicked() { self.is_favus_open = !self.is_favus_open; }
                    if ui.selectable_label(self.is_piano_roll_open, egui::RichText::new("🎹 PR").color(tg_color(self.is_piano_roll_open))).clicked() { self.is_piano_roll_open = !self.is_piano_roll_open; }
                    if ui.selectable_label(self.is_omnia_open, egui::RichText::new("🌀 FX").color(tg_color(self.is_omnia_open))).clicked() { self.is_omnia_open = !self.is_omnia_open; }
                    if ui.selectable_label(self.is_fluctus_open, egui::RichText::new("🔌 Synth").color(tg_color(self.is_fluctus_open))).clicked() { 
                        if !self.is_fluctus_open && self.selected_track.is_none() { self.selected_track = Some(0); }
                        self.is_fluctus_open = !self.is_fluctus_open; 
                    }
                    
                    ui.separator();
                    
                    // 3. Central Transport (Play/Stop with explicit colors)
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let center_offset = ui.available_width() / 2.0 - 60.0;
                        if center_offset > 0.0 { ui.add_space(center_offset); }
                        
                        let is_playing = graph.engine.is_playing;
                        if ui.add_sized([35.0, 26.0], egui::Button::new(egui::RichText::new("▶").size(16.0).color(if is_playing { egui::Color32::GREEN } else { egui::Color32::GRAY }))).clicked() {
                            graph.engine.is_playing = true;
                        }
                        if ui.add_sized([35.0, 26.0], egui::Button::new(egui::RichText::new("⏸").size(16.0).color(if !is_playing { egui::Color32::RED } else { egui::Color32::GRAY }))).clicked() {
                            graph.engine.is_playing = false;
                        }
                        if ui.add_sized([35.0, 26.0], egui::Button::new(egui::RichText::new("⏹").size(14.0).color(egui::Color32::GRAY))).clicked() {
                            graph.engine.is_playing = false;
                            graph.engine.current_step = 0;
                        }
                    });

                    // 4. Time / BPM Information
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        // Digital style step counter
                        let step_text = format!("{:03}.{:02}", graph.engine.current_step / 4 + 1, (graph.engine.current_step % 4) + 1);
                        ui.label(egui::RichText::new(step_text).strong().monospace().size(18.0).color(egui::Color32::from_rgb(0, 255, 150)));
                        ui.separator();
                        
                        let mut bpm = graph.engine.bpm;
                        ui.add(egui::DragValue::new(&mut bpm).range(20.0..=300.0).speed(1.0).suffix(" BPM"));
                        graph.engine.bpm = bpm;
                    });
                });
            });

            // Mixer / Track Overview (Left)
            egui::SidePanel::left("mixer_left").resizable(true).min_width(200.0).max_width(320.0).show(&self.egui_ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("🎚 MIXER").color(egui::Color32::from_rgb(200, 200, 200)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add").clicked() {
                            graph.engine.tracks.push(flumen_engine::graph::EngineTrack {
                                node: Box::new(flumen_engine::graph::PolySynth::new(flumen_engine::graph::Waveform::Saw)),
                                patterns: vec![flumen_engine::graph::Pattern::default()],
                                current_pattern_idx: 0,
                                volume: 0.8,
                                pan: 0.0,
                            });
                        }
                    });
                });
                ui.separator();
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(5.0);
                    let mut track_to_remove = None;

                    for (idx, track) in graph.engine.tracks.iter_mut().enumerate() {
                        let is_selected = self.selected_track == Some(idx);
                        
                        let bg_color = if is_selected { 
                            egui::Color32::from_rgb(30, 40, 50) 
                        } else { 
                            egui::Color32::from_rgb(20, 20, 25) 
                        };

                        let frame = egui::Frame::none()
                            .fill(bg_color)
                            .rounding(4.0)
                            .inner_margin(8.0)
                            .stroke(if is_selected { egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 255)) } else { egui::Stroke::NONE });
                        
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let label = format!("TRK {:02}", idx + 1);
                                if ui.selectable_label(is_selected, egui::RichText::new(label).strong().color(if is_selected { egui::Color32::WHITE } else { egui::Color32::GRAY })).clicked() {
                                    self.selected_track = if is_selected { None } else { Some(idx) };
                                }
                                
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("🗑").clicked() {
                                        track_to_remove = Some(idx);
                                    }
                                    let mut is_synth_open = self.is_fluctus_open && self.selected_track == Some(idx);
                                    if ui.toggle_value(&mut is_synth_open, "🔌").clicked() {
                                        if is_synth_open { self.selected_track = Some(idx); }
                                        self.is_fluctus_open = is_synth_open;
                                    }
                                });
                            });
                            
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                Self::custom_knob(ui, &mut track.volume, 0.0..=1.0, "VOL");
                                ui.add_space(10.0);
                                Self::custom_knob(ui, &mut track.pan, -1.0..=1.0, "PAN");
                            });
                        });
                        ui.add_space(4.0);
                    }
                    
                    if let Some(idx) = track_to_remove {
                        graph.engine.tracks.remove(idx);
                        if self.selected_track == Some(idx) {
                            self.selected_track = None;
                        } else if let Some(sel) = self.selected_track {
                            if sel > idx {
                                self.selected_track = Some(sel - 1);
                            }
                        }
                    }
                });
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
                            if let Some(synth) = track.node.as_any_mut().downcast_mut::<flumen_engine::graph::PolySynth>() {
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
                                
                                // Waveform Selector and Octave (Knobs)
                                ui.horizontal(|ui| {
                                    let mut waveform_idx = match synth.waveform {
                                        flumen_engine::graph::Waveform::Sine => 0.0,
                                        flumen_engine::graph::Waveform::Saw => 1.0,
                                        flumen_engine::graph::Waveform::Square => 2.0,
                                        flumen_engine::graph::Waveform::Triangle => 3.0,
                                    };
                                    if Self::custom_knob(ui, &mut waveform_idx, 0.0..=3.0, "OSC") {
                                        synth.set_waveform(match waveform_idx.round() as i32 {
                                            0 => flumen_engine::graph::Waveform::Sine,
                                            1 => flumen_engine::graph::Waveform::Saw,
                                            2 => flumen_engine::graph::Waveform::Square,
                                            _ => flumen_engine::graph::Waveform::Triangle,
                                        });
                                    }
                                    
                                    ui.add_space(8.0);
                                    let mut oct = synth.octave_offset as f32;
                                    if Self::custom_knob(ui, &mut oct, -3.0..=3.0, "OCT") {
                                        synth.octave_offset = oct.round() as i32;
                                    }

                                    ui.add_space(15.0);
                                    
                                    // Unison Controls
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            let mut u_count = synth.unison_count as f32;
                                            if Self::custom_knob(ui, &mut u_count, 1.0..=7.0, "UNISON") {
                                                synth.unison_count = u_count.round() as u32;
                                            }

                                            ui.add_space(5.0);
                                            Self::custom_knob(ui, &mut synth.unison_detune, 0.0..=1.0, "DETUNE");
                                            ui.add_space(5.0);
                                            Self::custom_knob(ui, &mut synth.unison_blend, 0.0..=1.0, "BLEND");
                                        });
                                    });
                                });
                                
                                ui.add_space(8.0);
                                ui.group(|ui| {
                                    ui.label("ADSR Envelope:");
                                    ui.horizontal_wrapped(|ui| {
                                        let mut adsr = synth.adsr;
                                        Self::custom_knob(ui, &mut adsr.attack, 0.001..=1.0, "ATK");
                                        Self::custom_knob(ui, &mut adsr.decay, 0.001..=1.0, "DEC");
                                        Self::custom_knob(ui, &mut adsr.sustain, 0.0..=1.0, "SUS");
                                        Self::custom_knob(ui, &mut adsr.release, 0.001..=2.0, "REL");
                                        if adsr != synth.adsr {
                                            synth.set_adsr(adsr);
                                        }
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
                                let _step_w = 10.0; // zoom_x for favus is different or fixed
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
                                        let _track_idx = (rel_y / track_h + self.favus_scroll.y) as usize;

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
                            // --- Piano Roll View ---
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                            // Minimalist Info Bar
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("🎹 PIANO_ROLL").strong().color(egui::Color32::from_rgb(0, 200, 255)));
                                ui.separator();
                                ui.label(format!("Length: {} steps", self.pr_note_length));
                                ui.separator();
                                ui.label(egui::RichText::new("Alt+Wheel: Note Size").size(10.0).color(egui::Color32::GRAY));
                                ui.separator();
                                ui.label(egui::RichText::new("Shift+Click: Select").size(10.0).color(egui::Color32::GRAY));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("🗑 Clear All").clicked() {
                                        if let Some(track_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get_mut(track_idx) {
                                                track.current_pattern_mut().grid = [[0; 128]; 120];
                                            }
                                        }
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            ui.separator();

                            let available = ui.available_size();
                            let grid_w = available.x - 16.0;
                            let grid_h = available.y - 24.0;

                            ui.horizontal(|ui| {
                                let (rect, response) = ui.allocate_exact_size(egui::vec2(grid_w, grid_h), egui::Sense::click_and_drag());
                                self.central_panel_rect = rect;
                                central_rect = rect;

                                let zoom_x = 60.0;
                                let zoom_y = 25.0;
                                let piano_key_width_ratio = 0.06;
                                let grid_x_start = rect.min.x + rect.width() * piano_key_width_ratio;

                                // --- KEYBOARD SHORTCUTS ---
                                if response.hovered() {
                                    if ui.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.ctrl) {
                                        if let Some(t_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get(t_idx) {
                                                self.pr_selected_notes.clear();
                                                let pattern = track.current_pattern();
                                                for p in 0..120 {
                                                    for s in 0..128 {
                                                        if pattern.grid[p][s] > 0 { self.pr_selected_notes.push((p, s)); }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if ui.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
                                        self.pr_selected_notes.clear();
                                    }
                                    if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                                        if let Some(t_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get_mut(t_idx) {
                                                let pattern = track.current_pattern_mut();
                                                for &(p, s) in &self.pr_selected_notes { pattern.grid[p][s] = 0; }
                                                self.pr_selected_notes.clear();
                                            }
                                        }
                                    }
                                    // Alt + Wheel to change default note length
                                    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
                                    if ui.input(|i| i.modifiers.alt) && scroll_delta.y.abs() > 0.1 {
                                        if scroll_delta.y > 0.0 { self.pr_note_length = (self.pr_note_length + 1).min(16); }
                                        else { self.pr_note_length = (self.pr_note_length.saturating_sub(1)).max(1); }
                                    }
                                }

                                // --- SCROLL ---
                                let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                                if ui.input(|i| i.modifiers.shift) {
                                    self.pr_scroll.x -= scroll_delta.y / zoom_x;
                                } else {
                                    self.pr_scroll.y -= scroll_delta.y / zoom_y;
                                }
                                self.pr_scroll.x -= scroll_delta.x / zoom_x;

                                if response.dragged_by(egui::PointerButton::Middle) {
                                    let delta = response.drag_delta();
                                    self.pr_scroll.x -= delta.x / zoom_x;
                                    self.pr_scroll.y -= delta.y / zoom_y;
                                }

                                // === HELPERS ===
                                let get_grid_coords = |pos: egui::Pos2| -> (i32, i32) {
                                    let y_idx = ((pos.y - rect.min.y) / zoom_y + self.pr_scroll.y) as i32;
                                    let pitch_idx = (119 - y_idx) as i32;
                                    let x_idx = ((pos.x - grid_x_start) / zoom_x + self.pr_scroll.x) as i32;
                                    (pitch_idx, x_idx)
                                };

                                let in_bounds = |pitch: i32, step: i32| -> bool {
                                    pitch >= 0 && pitch < 120 && step >= 0 && step < 128
                                };

                                // === MOUSE & INTERACTION ===
                                let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                                if let Some(pos) = pointer_pos {
                                    if rect.contains(pos) {
                                        let (pitch_idx, x_idx) = get_grid_coords(pos);
                                        let in_grid = pos.x > grid_x_start && in_bounds(pitch_idx, x_idx);
                                        let in_keys = pos.x <= grid_x_start && in_bounds(pitch_idx, 0);

                                        if let Some(t_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get_mut(t_idx) {
                                                if in_keys {
                                                    let midi = 12 + pitch_idx;
                                                    let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
                                                    if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                                        if self.mouse_played_note != Some(freq) {
                                                            if let Some(old_f) = self.mouse_played_note { track.node.release_freq(old_f); }
                                                            track.node.trigger_freq(freq);
                                                            self.mouse_played_note = Some(freq);
                                                        }
                                                    }
                                                } else if in_grid {
                                                    let p_idx = pitch_idx as usize;
                                                    let s_idx = x_idx as usize;
                                                    let pattern = track.current_pattern_mut();

                                                    // Detect note
                                                    let mut hovered = None;
                                                    for s in (0..=s_idx).rev() {
                                                        let l = pattern.grid[p_idx][s];
                                                        if l > 0 && (s + l as usize) > s_idx {
                                                            let offset = (pos.x - (grid_x_start + (s as f32 - self.pr_scroll.x) * zoom_x)) / zoom_x;
                                                            hovered = Some((p_idx, s, l, offset));
                                                            break;
                                                        }
                                                    }

                                                    // Cursor & State
                                                    if let Some((_, _, l, off)) = hovered {
                                                        if off > (l as f32 - 0.25) {
                                                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                        } else {
                                                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                                                        }
                                                    } else {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
                                                    }

                                                    // Actions
                                                    let is_primary_pressed = ui.input(|i| i.pointer.primary_pressed());
                                                    let is_primary_down = ui.input(|i| i.pointer.primary_down());
                                                    let is_secondary_down = ui.input(|i| i.pointer.secondary_down());

                                                    if is_primary_pressed {
                                                        if ui.input(|i| i.modifiers.ctrl) {
                                                            self.pr_selection = Some((egui::vec2(x_idx as f32, pitch_idx as f32), egui::vec2(x_idx as f32, pitch_idx as f32)));
                                                        } else if let Some((p, s, l, off)) = hovered {
                                                            self.pr_last_clicked_note = Some((p, s));
                                                            if off > (l as f32 - 0.25) { 
                                                                self.pr_resize_mode = true; 
                                                            } else { 
                                                                self.pr_is_dragging = true; 
                                                            }
                                                        } else {
                                                            // FL Studio Style Note Placement
                                                            pattern.grid[p_idx][s_idx] = self.pr_note_length;
                                                            self.pr_last_clicked_note = Some((p_idx, s_idx));
                                                            self.pr_resize_mode = true; // Automatically enter resize mode to drag length out
                                                        }
                                                    } else if is_primary_down {
                                                        if self.pr_is_dragging {
                                                            if let Some((old_p, old_s)) = self.pr_last_clicked_note {
                                                                if (old_p != p_idx || old_s != s_idx) && in_bounds(pitch_idx, x_idx) && pattern.grid[p_idx][s_idx] == 0 {
                                                                    let l = pattern.grid[old_p][old_s];
                                                                    pattern.grid[old_p][old_s] = 0;
                                                                    pattern.grid[p_idx][s_idx] = l;
                                                                    self.pr_last_clicked_note = Some((p_idx, s_idx));
                                                                }
                                                            }
                                                        } else if self.pr_resize_mode {
                                                            if let Some((p, s)) = self.pr_last_clicked_note {
                                                                let nl = (x_idx - s as i32 + 1).max(1).min(127) as u8;
                                                                pattern.grid[p][s] = nl;
                                                            }
                                                        }
                                                    }

                                                    // Right click continuous erase
                                                    if is_secondary_down {
                                                        if let Some((p, s, _, _)) = hovered { pattern.grid[p][s] = 0; }
                                                        else { pattern.grid[p_idx][s_idx] = 0; }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                                    self.pr_is_dragging = false;
                                    self.pr_resize_mode = false;
                                    self.pr_last_clicked_note = None;
                                    self.pr_draw_mode = None;
                                    if let Some(freq) = self.mouse_played_note.take() {
                                        if let Some(t_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get_mut(t_idx) { track.node.release_freq(freq); }
                                        }
                                    }
                                    // Area Selection Apply
                                    if let Some((start, end)) = self.pr_selection.take() {
                                        let (mx, mmx) = (start.x.min(end.x) as usize, start.x.max(end.x) as usize);
                                        let (my, mmy) = (start.y.min(end.y) as usize, start.y.max(end.y) as usize);
                                        if let Some(t_idx) = self.selected_track {
                                            if let Some(track) = graph.engine.tracks.get(t_idx) {
                                                self.pr_selected_notes.clear();
                                                for y in my..=mmy {
                                                    for x in mx..=mmx {
                                                        if y < 120 && x < 128 && track.current_pattern().grid[y][x] > 0 {
                                                            self.pr_selected_notes.push((y, x));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary)) && ui.input(|i| i.modifiers.ctrl) {
                                    if let Some(sel) = self.pr_selection.as_mut() {
                                        let (p, x) = get_grid_coords(ui.input(|i| i.pointer.interact_pos().unwrap_or(egui::pos2(0.0,0.0))));
                                        sel.1 = egui::vec2(x as f32, p as f32);
                                    }
                                }


                                // === DRAWING ===
                                let painter = ui.painter_at(rect);
                                
                                // 1. Render Notes Overlay (Labels/Glow)
                                if let Some(t_idx) = self.selected_track {
                                    if let Some(track) = graph.engine.tracks.get(t_idx) {
                                        let pattern = track.current_pattern();
                                        let start_p = (self.pr_scroll.y) as i32;
                                        let end_p = (start_p + (grid_h / zoom_y) as i32 + 2).min(120);
                                        let start_s = (self.pr_scroll.x) as i32;
                                        let end_s = (start_s + (grid_w / zoom_x) as i32 + 2).min(128);

                                        for p in (120 - end_p)..(120 - start_p) {
                                            let p_idx = p as usize;
                                            let riv = (119 - p) as f32 - self.pr_scroll.y;
                                            let ys = rect.min.y + riv * zoom_y;

                                            for s in start_s..end_s {
                                                let s_idx = s as usize;
                                                let len = pattern.grid[p_idx][s_idx];
                                                if len > 0 {
                                                    let civ = s as f32 - self.pr_scroll.x;
                                                    let nr = egui::Rect::from_min_size(
                                                        egui::pos2(grid_x_start + civ * zoom_x, ys),
                                                        egui::vec2(len as f32 * zoom_x, zoom_y)
                                                    ).shrink(1.0);
                                                    
                                                    let is_sel = self.pr_selected_notes.contains(&(p_idx, s_idx));
                                                    
                                                    // Beautiful gradients and strokes for commercial feel
                                                    let fill_color = if is_sel { egui::Color32::from_rgb(30, 200, 200) } else { egui::Color32::from_rgb(30, 120, 255) };
                                                    let stroke_color = if is_sel { egui::Color32::WHITE } else { egui::Color32::WHITE.gamma_multiply(0.4) };
                                                    
                                                    painter.rect_filled(nr, 3.0, fill_color);
                                                    painter.rect_stroke(nr, 3.0, egui::Stroke::new(1.0, stroke_color), egui::StrokeKind::Inside);
                                                    painter.rect_stroke(nr, 3.0, egui::Stroke::new(1.0, egui::Color32::BLACK.gamma_multiply(0.6)), egui::StrokeKind::Outside);
                                                    
                                                    // Drag handle marker on the right edge (Interactive indicator)
                                                    if nr.width() > 10.0 {
                                                        let handle_rect = egui::Rect::from_min_max(
                                                            egui::pos2(nr.max.x - 5.0, nr.min.y + 4.0),
                                                            egui::pos2(nr.max.x - 2.0, nr.max.y - 4.0),
                                                        );
                                                        painter.rect_filled(handle_rect, 1.0, egui::Color32::WHITE.gamma_multiply(0.5));
                                                    }
                                                    
                                                    // Centered, readable note name
                                                    if nr.width() > 25.0 {
                                                        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                                                        let name = format!("{}{}", names[p_idx % 12], p_idx / 12 - 1);
                                                        painter.text(
                                                            egui::pos2(nr.min.x + 6.0, nr.center().y),
                                                            egui::Align2::LEFT_CENTER,
                                                            name, 
                                                            egui::FontId::proportional(11.0), 
                                                            egui::Color32::WHITE.gamma_multiply(0.9)
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Keys & Names
                                for p in 0..120 {
                                    let riv = (119 - p) as f32 - self.pr_scroll.y;
                                    let ys = rect.min.y + riv * zoom_y;
                                    if ys > rect.min.y - zoom_y && ys < rect.max.y {
                                        let midi = 12 + p;
                                        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                                        let is_b = [1,3,6,8,10].contains(&(midi%12));
                                        painter.text(
                                            egui::pos2(rect.min.x + 4.0, ys + zoom_y/2.0),
                                            egui::Align2::LEFT_CENTER,
                                            format!("{}{}", names[midi%12], midi/12 - 1),
                                            egui::FontId::proportional(10.0),
                                            if is_b { egui::Color32::from_gray(80) } else { egui::Color32::from_gray(180) }
                                        );
                                    }
                                }

                                // Area Selection Rendering
                                if let Some((s, e)) = self.pr_selection {
                                    let sr = egui::Rect::from_min_max(
                                        egui::pos2(grid_x_start + (s.x.min(e.x) - self.pr_scroll.x) * zoom_x, rect.min.y + (119.0 - s.y.max(e.y) - self.pr_scroll.y) * zoom_y),
                                        egui::pos2(grid_x_start + (s.x.max(e.x) - self.pr_scroll.x + 1.0) * zoom_x, rect.min.y + (119.0 - s.y.min(e.y) - self.pr_scroll.y + 1.0) * zoom_y)
                                    );
                                    painter.rect_filled(sr, 0.0, egui::Color32::from_rgba_unmultiplied(0, 150, 255, 40));
                                    painter.rect_stroke(sr, 0.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 255)), egui::StrokeKind::Inside);
                                }

                                // Selection Glow
                                if let Some(t_idx) = self.selected_track {
                                    if let Some(track) = graph.engine.tracks.get(t_idx) {
                                        for &(p, s) in &self.pr_selected_notes {
                                            let l = track.current_pattern().grid[p][s];
                                            let riv = (119 - p) as f32 - self.pr_scroll.y;
                                            let civ = s as f32 - self.pr_scroll.x;
                                            let nr = egui::Rect::from_min_size(
                                                egui::pos2(grid_x_start + civ * zoom_x, rect.min.y + riv * zoom_y),
                                                egui::vec2(l as f32 * zoom_x, zoom_y)
                                            ).shrink(1.0);
                                            if rect.intersects(nr) {
                                                painter.rect_stroke(nr.expand(2.0), 2.0, egui::Stroke::new(1.5, egui::Color32::WHITE.gamma_multiply(0.3)), egui::StrokeKind::Outside);
                                            }
                                        }
                                    }
                                }

                                // Sliders
                                ui.spacing_mut().slider_width = grid_h;
                                let mut scroll_inv_y = 120.0 - self.pr_scroll.y;
                                ui.add(egui::Slider::new(&mut scroll_inv_y, 0.0..=120.0).vertical().show_value(false));
                                self.pr_scroll.y = 120.0 - scroll_inv_y;
                            });

                            ui.horizontal(|ui| {
                                ui.add_space(grid_w * 0.06); 
                                ui.spacing_mut().slider_width = grid_w * 0.94;
                                let mut scroll_x = self.pr_scroll.x;
                                ui.add(egui::Slider::new(&mut scroll_x, 0.0..=128.0).show_value(false));
                                self.pr_scroll.x = scroll_x;
                            });

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
