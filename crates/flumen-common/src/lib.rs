pub mod plugin {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PluginInfo {
        pub id: String,
        pub name: String,
        pub version: String,
        pub author: String,
        pub description: String,
        pub plugin_type: PluginType,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub enum PluginType {
        Effect,
        Instrument,
        Analyzer,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PluginParam {
        pub name: String,
        pub label: String,
        pub min: f32,
        pub max: f32,
        pub default: f32,
        pub value: f32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct PluginManifest {
        pub info: PluginInfo,
        pub params: Vec<PluginParam>,
        pub library: String,
    }
}

pub mod window {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum PanelType {
        Mixer,
        PianoRoll,
        Favus,
        Fluctus,
        Omnia,
        PluginManager,
        Settings,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum LayoutMode {
        Tiling,
        Floating,
        Tabbed,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PanelState {
        pub panel_type: PanelType,
        pub visible: bool,
        pub x: f32,
        pub y: f32,
        pub width: f32,
        pub height: f32,
        pub floating: bool,
    }

    impl PanelState {
        pub fn new(panel_type: PanelType, x: f32, y: f32, width: f32, height: f32) -> Self {
            Self {
                panel_type,
                visible: true,
                x,
                y,
                width,
                height,
                floating: false,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WindowManager {
        pub panels: Vec<PanelState>,
        pub layout: LayoutMode,
        pub focused_panel: Option<usize>,
    }

    impl WindowManager {
        pub fn default_layout() -> Self {
            let panels = vec![
                PanelState::new(PanelType::Mixer, 0.0, 0.0, 1.0, 0.25),
                PanelState::new(PanelType::Favus, 0.0, 0.25, 1.0, 0.35),
                PanelState::new(PanelType::PianoRoll, 0.0, 0.6, 0.6, 0.4),
                PanelState::new(PanelType::Fluctus, 0.6, 0.6, 0.4, 0.2),
                PanelState::new(PanelType::Omnia, 0.6, 0.8, 0.4, 0.2),
            ];

            Self {
                panels,
                layout: LayoutMode::Tiling,
                focused_panel: None,
            }
        }

        pub fn toggle_panel(&mut self, panel_type: PanelType) {
            if let Some(panel) = self.panels.iter_mut().find(|p| p.panel_type == panel_type) {
                panel.visible = !panel.visible;
            }
        }

        pub fn focus_panel(&mut self, idx: usize) {
            self.focused_panel = Some(idx);
        }

        pub fn next_panel(&mut self) {
            let visible: Vec<usize> = self.panels.iter().enumerate()
                .filter(|(_, p)| p.visible)
                .map(|(i, _)| i)
                .collect();
            
            if visible.is_empty() { return; }

            if let Some(current) = self.focused_panel {
                if let Some(pos) = visible.iter().position(|&i| i == current) {
                    let next = (pos + 1) % visible.len();
                    self.focused_panel = Some(visible[next]);
                } else {
                    self.focused_panel = Some(visible[0]);
                }
            } else {
                self.focused_panel = Some(visible[0]);
            }
        }

        pub fn prev_panel(&mut self) {
            let visible: Vec<usize> = self.panels.iter().enumerate()
                .filter(|(_, p)| p.visible)
                .map(|(i, _)| i)
                .collect();
            
            if visible.is_empty() { return; }

            if let Some(current) = self.focused_panel {
                if let Some(pos) = visible.iter().position(|&i| i == current) {
                    let prev = if pos == 0 { visible.len() - 1 } else { pos - 1 };
                    self.focused_panel = Some(visible[prev]);
                } else {
                    self.focused_panel = Some(visible[0]);
                }
            } else {
                self.focused_panel = Some(visible[0]);
            }
        }

        pub fn get_panel_rect(&self, idx: usize, screen_w: f32, screen_h: f32) -> (f32, f32, f32, f32) {
            if let Some(panel) = self.panels.get(idx) {
                if panel.floating {
                    (panel.x, panel.y, panel.width, panel.height)
                } else {
                    (panel.x * screen_w, panel.y * screen_h, panel.width * screen_w, panel.height * screen_h)
                }
            } else {
                (0.0, 0.0, screen_w, screen_h)
            }
        }
    }
}

pub mod project {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
    pub enum Waveform {
        Sine,
        Saw,
        Square,
        Triangle,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct AdsrEnvelope {
        pub attack: f32,
        pub decay: f32,
        pub sustain: f32,
        pub release: f32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct SynthParams {
        pub waveform: Waveform,
        pub adsr: AdsrEnvelope,
        pub unison_count: u32,
        pub unison_detune: f32,
        pub unison_blend: f32,
        pub octave_offset: i32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct OmniaParams {
        pub delay_time: f32,
        pub delay_feedback: f32,
        pub delay_mix: f32,
        pub reverb_size: f32,
        pub reverb_mix: f32,
        pub eq_low: f32,
        pub eq_high: f32,
        pub dist_drive: f32,
        pub dist_mix: f32,
        pub is_enabled: bool,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Pattern {
        pub grid: Vec<Vec<u8>>, // 120 notes x 128 steps
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ArrangementItem {
        pub track_id: u32,
        pub pattern_index: usize,
        pub start_step: u32,
        pub length: u32,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Track {
        pub id: u32,
        pub name: String,
        pub volume: f32,
        pub panned: f32,
        pub synth: SynthParams,
        pub patterns: Vec<Pattern>,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct Project {
        pub name: String,
        pub bpm: f32,
        pub tracks: Vec<Track>,
        pub playlist: Vec<ArrangementItem>,
        pub omnia: OmniaParams,
    }
}
