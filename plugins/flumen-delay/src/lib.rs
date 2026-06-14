use flumen_common::plugin::{PluginManifest, PluginInfo, PluginType, PluginParam};
use flumen_engine::graph::{AudioNode, ProcessContext};

pub struct DelayPlugin {
    params: Vec<PluginParam>,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    ptr: usize,
    time: f32,
    feedback: f32,
    mix: f32,
}

impl DelayPlugin {
    pub fn new() -> Self {
        let size = 48000 * 2;
        Self {
            params: vec![
                PluginParam {
                    name: "time".to_string(),
                    label: "Time".to_string(),
                    min: 0.01,
                    max: 2.0,
                    default: 0.3,
                    value: 0.3,
                },
                PluginParam {
                    name: "feedback".to_string(),
                    label: "Feedback".to_string(),
                    min: 0.0,
                    max: 0.95,
                    default: 0.4,
                    value: 0.4,
                },
                PluginParam {
                    name: "mix".to_string(),
                    label: "Mix".to_string(),
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    value: 0.5,
                },
            ],
            buffer_l: vec![0.0; size],
            buffer_r: vec![0.0; size],
            ptr: 0,
            time: 0.3,
            feedback: 0.4,
            mix: 0.5,
        }
    }

    fn update_from_params(&mut self) {
        for param in &self.params {
            match param.name.as_str() {
                "time" => self.time = param.value,
                "feedback" => self.feedback = param.value,
                "mix" => self.mix = param.value,
                _ => {}
            }
        }
    }
}

impl AudioNode for DelayPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        self.update_from_params();
        
        let size = self.buffer_l.len();
        let delay_samples = (self.time * context.sample_rate).clamp(1.0, size as f32 - 1.0) as usize;
        let read_ptr = (self.ptr + size - delay_samples) % size;
        
        for sample in outputs[0].iter_mut() {
            let delayed_l = self.buffer_l[read_ptr];
            let delayed_r = self.buffer_r[read_ptr];
            
            self.buffer_l[self.ptr] = *sample + delayed_l * self.feedback;
            self.buffer_r[self.ptr] = *sample + delayed_r * self.feedback;
            self.ptr = (self.ptr + 1) % size;
            
            *sample = *sample * (1.0 - self.mix) + delayed_l * self.mix;
        }
    }
}

#[no_mangle]
pub extern "C" fn flumen_create_plugin() -> Box<dyn AudioNode> {
    Box::new(DelayPlugin::new())
}

#[no_mangle]
pub extern "C" fn flumen_plugin_info() -> PluginManifest {
    PluginManifest {
        info: PluginInfo {
            id: "flumen-delay".to_string(),
            name: "Flumen Delay".to_string(),
            version: "0.1.0".to_string(),
            author: "NLPA Collab".to_string(),
            description: "Simple stereo delay effect".to_string(),
            plugin_type: PluginType::Effect,
        },
        params: vec![
            PluginParam {
                name: "time".to_string(),
                label: "Time".to_string(),
                min: 0.01,
                max: 2.0,
                default: 0.3,
                value: 0.3,
            },
            PluginParam {
                name: "feedback".to_string(),
                label: "Feedback".to_string(),
                min: 0.0,
                max: 0.95,
                default: 0.4,
                value: 0.4,
            },
            PluginParam {
                name: "mix".to_string(),
                label: "Mix".to_string(),
                min: 0.0,
                max: 1.0,
                default: 0.5,
                value: 0.5,
            },
        ],
        library: "flumen_delay".to_string(),
    }
}
