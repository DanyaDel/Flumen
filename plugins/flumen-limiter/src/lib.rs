use flumen_common::plugin::{PluginManifest, PluginInfo, PluginType, PluginParam};
use flumen_engine::graph::{AudioNode, ProcessContext};

pub struct LimiterPlugin {
    params: Vec<PluginParam>,
    ceiling: f32,
    release: f32,
    envelope: f32,
}

impl LimiterPlugin {
    pub fn new() -> Self {
        Self {
            params: vec![
                PluginParam {
                    name: "ceiling".to_string(),
                    label: "Ceiling".to_string(),
                    min: -12.0,
                    max: 0.0,
                    default: -0.3,
                    value: -0.3,
                },
                PluginParam {
                    name: "release".to_string(),
                    label: "Release".to_string(),
                    min: 1.0,
                    max: 100.0,
                    default: 10.0,
                    value: 10.0,
                },
            ],
            ceiling: -0.3,
            release: 10.0,
            envelope: 0.0,
        }
    }

    fn update_from_params(&mut self) {
        for param in &self.params {
            match param.name.as_str() {
                "ceiling" => self.ceiling = param.value,
                "release" => self.release = param.value,
                _ => {}
            }
        }
    }

    fn db_to_linear(&self, db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }
}

impl AudioNode for LimiterPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        self.update_from_params();
        
        let ceiling_linear = self.db_to_linear(self.ceiling);
        let release_coeff = (-1.0 / (self.release * 0.001 * context.sample_rate)).exp();
        
        for sample in outputs[0].iter_mut() {
            let input_abs = sample.abs();
            
            if input_abs > self.envelope {
                self.envelope = input_abs;
            } else {
                self.envelope = self.envelope * release_coeff;
            }
            
            if self.envelope > ceiling_linear {
                *sample *= ceiling_linear / self.envelope;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn flumen_create_plugin() -> Box<dyn AudioNode> {
    Box::new(LimiterPlugin::new())
}

#[no_mangle]
pub extern "C" fn flumen_plugin_info() -> PluginManifest {
    PluginManifest {
        info: PluginInfo {
            id: "flumen-limiter".to_string(),
            name: "Flumen Limiter".to_string(),
            version: "0.1.0".to_string(),
            author: "NLPA Collab".to_string(),
            description: "Simple peak limiter".to_string(),
            plugin_type: PluginType::Effect,
        },
        params: vec![
            PluginParam {
                name: "ceiling".to_string(),
                label: "Ceiling".to_string(),
                min: -12.0,
                max: 0.0,
                default: -0.3,
                value: -0.3,
            },
            PluginParam {
                name: "release".to_string(),
                label: "Release".to_string(),
                min: 1.0,
                max: 100.0,
                default: 10.0,
                value: 10.0,
            },
        ],
        library: "flumen_limiter".to_string(),
    }
}
