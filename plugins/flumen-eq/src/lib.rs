use flumen_common::plugin::{PluginManifest, PluginInfo, PluginType, PluginParam};
use flumen_engine::graph::{AudioNode, ProcessContext};

pub struct EqPlugin {
    params: Vec<PluginParam>,
    low_gain: f32,
    mid_gain: f32,
    high_gain: f32,
    low_state: f32,
    mid_state: f32,
    high_state: f32,
}

impl EqPlugin {
    pub fn new() -> Self {
        Self {
            params: vec![
                PluginParam {
                    name: "low".to_string(),
                    label: "Low".to_string(),
                    min: -12.0,
                    max: 12.0,
                    default: 0.0,
                    value: 0.0,
                },
                PluginParam {
                    name: "mid".to_string(),
                    label: "Mid".to_string(),
                    min: -12.0,
                    max: 12.0,
                    default: 0.0,
                    value: 0.0,
                },
                PluginParam {
                    name: "high".to_string(),
                    label: "High".to_string(),
                    min: -12.0,
                    max: 12.0,
                    default: 0.0,
                    value: 0.0,
                },
            ],
            low_gain: 0.0,
            mid_gain: 0.0,
            high_gain: 0.0,
            low_state: 0.0,
            mid_state: 0.0,
            high_state: 0.0,
        }
    }

    fn update_from_params(&mut self) {
        for param in &self.params {
            match param.name.as_str() {
                "low" => self.low_gain = param.value,
                "mid" => self.mid_gain = param.value,
                "high" => self.high_gain = param.value,
                _ => {}
            }
        }
    }

    fn db_to_linear(&self, db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }
}

impl AudioNode for EqPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        self.update_from_params();
        
        let low_freq = 200.0;
        let mid_freq = 1000.0;
        let high_freq = 5000.0;
        
        let low_coeff = (-2.0 * std::f32::consts::PI * low_freq / context.sample_rate).exp();
        let mid_coeff = (-2.0 * std::f32::consts::PI * mid_freq / context.sample_rate).exp();
        let high_coeff = (-2.0 * std::f32::consts::PI * high_freq / context.sample_rate).exp();
        
        let low_linear = self.db_to_linear(self.low_gain);
        let mid_linear = self.db_to_linear(self.mid_gain);
        let high_linear = self.db_to_linear(self.high_gain);
        
        for sample in outputs[0].iter_mut() {
            self.low_state = self.low_state * low_coeff + *sample * (1.0 - low_coeff);
            self.mid_state = self.mid_state * mid_coeff + *sample * (1.0 - mid_coeff);
            self.high_state = self.high_state * high_coeff + *sample * (1.0 - high_coeff);
            
            let low_out = self.low_state * low_linear;
            let mid_out = self.mid_state * mid_linear;
            let high_out = self.high_state * high_linear;
            
            *sample = low_out + mid_out + high_out;
        }
    }
}

#[no_mangle]
pub extern "C" fn flumen_create_plugin() -> Box<dyn AudioNode> {
    Box::new(EqPlugin::new())
}

#[no_mangle]
pub extern "C" fn flumen_plugin_info() -> PluginManifest {
    PluginManifest {
        info: PluginInfo {
            id: "flumen-eq".to_string(),
            name: "Flumen EQ".to_string(),
            version: "0.1.0".to_string(),
            author: "NLPA Collab".to_string(),
            description: "Simple 3-band equalizer".to_string(),
            plugin_type: PluginType::Effect,
        },
        params: vec![
            PluginParam {
                name: "low".to_string(),
                label: "Low".to_string(),
                min: -12.0,
                max: 12.0,
                default: 0.0,
                value: 0.0,
            },
            PluginParam {
                name: "mid".to_string(),
                label: "Mid".to_string(),
                min: -12.0,
                max: 12.0,
                default: 0.0,
                value: 0.0,
            },
            PluginParam {
                name: "high".to_string(),
                label: "High".to_string(),
                min: -12.0,
                max: 12.0,
                default: 0.0,
                value: 0.0,
            },
        ],
        library: "flumen_eq".to_string(),
    }
}
