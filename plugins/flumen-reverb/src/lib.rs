use flumen_common::plugin::{PluginManifest, PluginInfo, PluginType, PluginParam};
use flumen_engine::graph::{AudioNode, ProcessContext};

pub struct ReverbPlugin {
    params: Vec<PluginParam>,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    ptr: usize,
    decay: f32,
    mix: f32,
}

impl ReverbPlugin {
    pub fn new() -> Self {
        let size = 48000 * 2;
        Self {
            params: vec![
                PluginParam {
                    name: "size".to_string(),
                    label: "Room Size".to_string(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    value: 0.5,
                },
                PluginParam {
                    name: "decay".to_string(),
                    label: "Decay".to_string(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    value: 0.5,
                },
                PluginParam {
                    name: "mix".to_string(),
                    label: "Mix".to_string(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.3,
                    value: 0.3,
                },
            ],
            buffer_l: vec![0.0; size],
            buffer_r: vec![0.0; size],
            ptr: 0,
            decay: 0.5,
            mix: 0.3,
        }
    }

    fn update_from_params(&mut self) {
        for param in &self.params {
            match param.name.as_str() {
                "decay" => self.decay = param.value,
                "mix" => self.mix = param.value,
                _ => {}
            }
        }
    }
}

impl AudioNode for ReverbPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, outputs: &mut [&mut [f32]], _context: &ProcessContext) {
        self.update_from_params();
        
        let size = self.buffer_l.len();
        let room_size = (self.params[0].value * 0.9 + 0.1) * size as f32;
        let delay = room_size as usize;
        
        for sample in outputs[0].iter_mut() {
            let read_ptr = (self.ptr + size - delay) % size;
            
            let wet_l = self.buffer_l[read_ptr];
            let wet_r = self.buffer_r[read_ptr];
            
            self.buffer_l[self.ptr] = *sample + wet_l * self.decay;
            self.buffer_r[self.ptr] = *sample + wet_r * self.decay;
            self.ptr = (self.ptr + 1) % size;
            
            *sample = *sample * (1.0 - self.mix) + (wet_l + wet_r) * 0.5 * self.mix;
        }
    }
}

#[no_mangle]
pub extern "C" fn flumen_create_plugin() -> Box<dyn AudioNode> {
    Box::new(ReverbPlugin::new())
}

#[no_mangle]
pub extern "C" fn flumen_plugin_info() -> PluginManifest {
    PluginManifest {
        info: PluginInfo {
            id: "flumen-reverb".to_string(),
            name: "Flumen Reverb".to_string(),
            version: "0.1.0".to_string(),
            author: "NLPA Collab".to_string(),
            description: "Simple algorithmic reverb effect".to_string(),
            plugin_type: PluginType::Effect,
        },
        params: vec![
            PluginParam {
                name: "size".to_string(),
                label: "Room Size".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                value: 0.5,
            },
            PluginParam {
                name: "decay".to_string(),
                label: "Decay".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                value: 0.5,
            },
            PluginParam {
                name: "mix".to_string(),
                label: "Mix".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.3,
                value: 0.3,
            },
        ],
        library: "flumen_reverb".to_string(),
    }
}
