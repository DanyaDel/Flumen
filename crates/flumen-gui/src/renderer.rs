use egui_wgpu::wgpu;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                }
            ]
        }
    }
}

pub struct Renderer {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Создаем пустой буфер, который будем обновлять каждый кадр (для демо)
        // Создаем буфер с запасом (около 2МБ для ~80к вершин)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 1024 * 1024 * 4, // 4MB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            vertex_buffer,
            num_vertices: 0,
        }
    }

    pub fn update_grid(&mut self, 
        queue: &wgpu::Queue, 
        tracks: &[flumen_engine::graph::EngineTrack], 
        selected_idx: Option<usize>, 
        current_step: usize,
        scroll: egui::Vec2,
        visible_notes: f32,
        visible_steps: f32,
    ) {
        let mut vertices = Vec::new();
        
        let total_notes = 120;
        let total_steps = 128;
        let spacing = 0.005;
        // 0.06 ratio * 2.0 NDC width = 0.12
        let piano_key_width = 0.12; 
        let grid_x_start = -1.0 + piano_key_width;
        
        let visible_steps = visible_steps.max(1.0);
        let visible_notes = visible_notes.max(1.0);
        
        // Width and height of cells in NDC based on visible area
        let step_width = (2.0 - piano_key_width - (visible_steps + 1.0) * spacing) / visible_steps;
        let step_height = (2.0 - (visible_notes + 1.0) * spacing) / visible_notes;
        
        let track = selected_idx.and_then(|idx| tracks.get(idx));
        
        // --- 1. Draw Piano Keys & Grid Rows ---
        // scroll.y = 0 means top of grid (pitch 119)
        let top_pitch_idx = (total_notes as i64 - 1).saturating_sub(scroll.y.floor() as i64);
        let visible_notes_i = (visible_notes.ceil() as i64).saturating_add(2);
        let bottom_pitch_idx = top_pitch_idx.saturating_sub(visible_notes_i);
        
        let start_pitch = bottom_pitch_idx.clamp(0, total_notes as i64) as i32;
        let end_pitch = (top_pitch_idx + 1).clamp(0, total_notes as i64) as i32;
        
        let start_step_i64 = scroll.x.floor() as i64;
        let end_step_i64 = (start_step_i64.saturating_add(visible_steps.ceil() as i64).saturating_add(2)).min(total_steps as i64);
        
        let start_step = start_step_i64.clamp(0, total_steps as i64) as i32;
        let end_step = end_step_i64.clamp(0, total_steps as i64) as i32;

        for pitch in start_pitch..end_pitch {
            let pitch_usize = pitch as usize;
            let pitch_i64 = pitch as i64;
            // Screen Y calculation: pitch 119 is at normalized Y = 0 (top of viewport) if scroll.y = 0
            // but in NDC Y=1 is top, Y=-1 is bottom. 
            // row_in_view: 0 is top cell, 1 is next cell down...
            let row_in_view = (total_notes as i64 - 1 - pitch_i64) as f32 - scroll.y;
            let y_start = 1.0 - row_in_view * (step_height + spacing);
            let y_end = y_start - step_height;
            
            // Piano Key color (midi 12 + pitch)
            let midi = 12 + pitch;
            let is_black = match midi % 12 {
                1 | 3 | 6 | 8 | 10 => true,
                _ => false,
            };
            
            // Draw Key
            let key_color = if is_black { [0.05, 0.05, 0.05] } else { [0.9, 0.9, 0.9] };
            self.write_rect(&mut vertices, [-1.0, y_start, grid_x_start - spacing, y_end], key_color);
            
            // --- Draw Grid Cells for this Pitch ---
            for step in start_step..end_step {
                let step_usize = step as usize;
                let col_in_view = step as f32 - scroll.x;
                let x_start = grid_x_start + (col_in_view as f32) * (step_width + spacing);
                let x_end = x_start + step_width;
                
                let len = if let Some(t) = track {
                    t.current_pattern().grid[pitch_usize][step_usize]
                } else {
                    0
                };
                    
                    if len > 0 {
                        // Draw note with length
                        let note_x_end = x_start + (len as f32) * (step_width + spacing) - spacing;
                        self.write_rect(&mut vertices, [x_start, y_start, note_x_end, y_end], [0.0, 0.8, 1.0]);
                    } else {
                        // Background cell
                        let base_color = if is_black {
                            [0.1, 0.1, 0.12]
                        } else {
                            [0.15, 0.15, 0.18]
                        };
                        let color = if step % 4 == 0 {
                            [base_color[0] + 0.03, base_color[1] + 0.03, base_color[2] + 0.03]
                        } else {
                            base_color
                        };
                        self.write_rect(&mut vertices, [x_start, y_start, x_end, y_end], color);
                    }
                }
            }

        // --- 2. Draw Marker Lines ---
        let marker_color = [0.25, 0.25, 0.28];
        let marker_width = 0.005;
        for i in (0..=total_steps).step_by(4) {
            let col_in_view = i as f32 - scroll.x;
            let x = grid_x_start + (col_in_view as f32) * (step_width + spacing) - spacing/2.0;
            if x > grid_x_start && x < 1.0 {
                self.write_rect(&mut vertices, [x - marker_width/2.0, 1.0, x + marker_width/2.0, -1.0], marker_color);
            }
        }

        // --- 3. Draw Playhead ---
        let ph_color = [0.0, 1.0, 0.6];
        let ph_width = 0.008;
        let ph_col_in_view = current_step as f32 - scroll.x;
        let ph_x = grid_x_start + (ph_col_in_view as f32) * (step_width + spacing) + step_width/2.0 - ph_width/2.0;
        if ph_x > grid_x_start && ph_x < 1.0 {
            self.write_rect(&mut vertices, [ph_x, 1.0, ph_x + ph_width, -1.0], ph_color);
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        self.num_vertices = vertices.len() as u32;
    }

    fn write_rect(&self, vertices: &mut Vec<Vertex>, rect: [f32; 4], color: [f32; 3]) {
        let [x1, y1, x2, y2] = rect;
        // Triangle 1
        vertices.push(Vertex { position: [x1, y1, 0.0], color });
        vertices.push(Vertex { position: [x1, y2, 0.0], color });
        vertices.push(Vertex { position: [x2, y2, 0.0], color });
        // Triangle 2
        vertices.push(Vertex { position: [x1, y1, 0.0], color });
        vertices.push(Vertex { position: [x2, y2, 0.0], color });
        vertices.push(Vertex { position: [x2, y1, 0.0], color });
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.num_vertices, 0..1);
    }
}
