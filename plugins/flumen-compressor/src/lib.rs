use flumen_common::plugin::{PluginManifest, PluginInfo, PluginType, PluginParam};
use flumen_engine::graph::{AudioNode, ProcessContext};

pub struct CompressorPlugin {
    params: Vec<PluginParam>,
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
    envelope: f32,
}

impl CompressorPlugin {
    pub fn new() -> Self {
        Self {
            params: vec![
                PluginParam {
                    name: "threshold".to_string(),
                    label: "Threshold".to_string(),
                    min: -60.0,
                    max: 0.0,
                    default: -20.0,
                    value: -20.0,
                },
                PluginParam {
                    name: "ratio".to_string(),
                    label: "Ratio".to_string(),
                    min: 1.0,
                    max: 20.0,
                    default: 4.0,
                    value: 4.0,
                },
                PluginParam {
                    name: "attack".to_string(),
                    label: "Attack".to_string(),
                    min: 0.1,
                    max: 100.0,
                    default: 10.0,
                    value: 10.0,
                },
                PluginParam {
                    name: "release".to_string(),
                    label: "Release".to_string(),
                    min: 10.0,
                    max: 1000.0,
                    default: 100.0,
                    value: 100.0,
                },
                PluginParam {
                    name: "makeup".to_string(),
                    label: "Makeup".to_string(),
                    min: 0.0,
                    max: 24.0,
                    default: 0.0,
                    value: 0.0,
                },
            ],
            threshold: -20.0,
            ratio: 4.0,
            attack: 10.0,
            release: 100.0,
            makeup: 0.0,
            envelope: 0.0,
        }
    }

    fn update_from_params(&mut self) {
        for param in &self.params {
            match param.name.as_str() {
                "threshold" => self.threshold = param.value,
                "ratio" => self.ratio = param.value,
                "attack" => self.attack = param.value,
                "release" => self.release = param.value,
                "makeup" => self.makeup = param.value,
                _ => {}
            }
        }
    }

    fn db_to_linear(&self, db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    fn linear_to_db(&self, linear: f32) -> f32 {
        20.0 * linear.max(1e-10).log10()
    }
}

impl AudioNode for CompressorPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, outputs: &mut [&mut [f32]], context: &ProcessContext) {
        self.update_from_params();
        
        let attack_coeff = (-1.0 / (self.attack * 0.001 * context.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * 0.001 * context.sample_rate)).exp();
        let makeup_linear = self.db_to_linear(self.makeup);
        
        for sample in outputs[0].iter_mut() {
            let input_db = self.linear_to_db(sample.abs());
            let over_db = (input_db - self.threshold).max(0.0);
            let compressed_db = over_db * (1.0 - 1.0 / self.ratio);
            
            let target_gain_db = -compressed_db;
            let target_gain = self.db_to_linear(target_gain_db);
            
            let coeff = if target_gain < self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            
            self.envelope = self.envelope * coeff + target_gain * (1.0 - coeff);
            
            *sample *= self.envelope * makeup_linear;
        }
    }
}

#[no_mangle]
pub extern "C" fn flumen_create_plugin() -> Box<dyn AudioNode> {
    Box::new(CompressorPlugin::new())
}

#[no_mangle]
pub extern "C" fn flumen_plugin_info() -> PluginManifest {
    PluginManifest {
        info: PluginInfo {
            id: "flumen-compressor".to_string(),
            name: "Flumen Compressor".to_string(),
            version: "0.1.0".to_string(),
            author: "NLPA Collab".to_string(),
            description: "Simple dynamics compressor".to_string(),
            plugin_type: PluginType::Effect,
        },
        params: vec![
            PluginParam {
                name: "threshold".to_string(),
                label: "Threshold".to_string(),
                min: -60.0,
                max: 0.0,
                default: -20.0,
                value: -20.0,
            },
            PluginParam {
                name: "ratio".to_string(),
                label: "Ratio".to_string(),
                min: 1.0,
                max: 20.0,
                default: 4.0,
                value: 4.0,
            },
            PluginParam {
                name: "attack".to_string(),
                label: "Attack".to_string(),
                min: 0.1,
                max: 100.0,
                default: 10.0,
                value: 10.0,
            },
            PluginParam {
                name: "release".to_string(),
                label: "Release".to_string(),
                min: 10.0,
                max: 1000.0,
                default: 100.0,
                value: 100.0,
            },
            PluginParam {
                name: "makeup".to_string(),
                label: "Makeup".to_string(),
                min: 0.0,
                max: 24.0,
                default: 0.0,
                value: 0.0,
            },
        ],
        library: "flumen_compressor".to_string(),
    }
}
