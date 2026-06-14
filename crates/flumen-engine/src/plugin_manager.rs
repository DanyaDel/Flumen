use std::collections::HashMap;
use std::path::PathBuf;
use flumen_common::plugin::PluginManifest;
use crate::graph::{AudioNode, ProcessContext};

pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub instance: Box<dyn AudioNode>,
    _lib: libloading::Library,
}

pub struct PluginManager {
    plugins_dir: PathBuf,
    loaded: HashMap<String, LoadedPlugin>,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self {
            plugins_dir,
            loaded: HashMap::new(),
        }
    }

    pub fn discover(&self) -> Vec<PluginManifest> {
        let mut manifests = Vec::new();
        
        if !self.plugins_dir.exists() {
            return manifests;
        }

        if let Ok(entries) = std::fs::read_dir(&self.plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let manifest_path = path.join("manifest.toml");
                    if manifest_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                            if let Ok(manifest) = toml::from_str::<PluginManifest>(&content) {
                                manifests.push(manifest);
                            }
                        }
                    }
                }
            }
        }

        manifests
    }

    pub fn load(&mut self, plugin_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.loaded.contains_key(plugin_id) {
            return Ok(());
        }

        let plugin_dir = self.plugins_dir.join(plugin_id);
        let manifest_path = plugin_dir.join("manifest.toml");
        
        if !manifest_path.exists() {
            return Err(format!("Plugin {} not found", plugin_id).into());
        }

        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;

        let lib_name = &manifest.library;
        #[cfg(target_os = "linux")]
        let lib_path = plugin_dir.join(format!("lib{}.so", lib_name));
        #[cfg(target_os = "macos")]
        let lib_path = plugin_dir.join(format!("lib{}.dylib", lib_name));
        #[cfg(target_os = "windows")]
        let lib_path = plugin_dir.join(format!("{}.dll", lib_name));

        unsafe {
            let lib = libloading::Library::new(&lib_path)?;
            
            type CreatePlugin = fn() -> Box<dyn AudioNode>;
            let create: libloading::Symbol<CreatePlugin> = lib.get(b"flumen_create_plugin")?;
            let instance = create();

            let loaded = LoadedPlugin {
                manifest,
                instance,
                _lib: lib,
            };

            self.loaded.insert(plugin_id.to_string(), loaded);
        }

        Ok(())
    }

    pub fn unload(&mut self, plugin_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.loaded.remove(plugin_id)
            .ok_or_else(|| -> Box<dyn std::error::Error> { 
                format!("Plugin {} not loaded", plugin_id).into() 
            })?;
        Ok(())
    }

    pub fn get(&self, plugin_id: &str) -> Option<&dyn AudioNode> {
        self.loaded.get(plugin_id).map(|p| p.instance.as_ref())
    }

    pub fn get_mut(&mut self, plugin_id: &str) -> Option<&mut dyn AudioNode> {
        self.loaded.get_mut(plugin_id).map(|p| p.instance.as_mut())
    }

    pub fn list_loaded(&self) -> Vec<&str> {
        self.loaded.keys().map(|s| s.as_str()).collect()
    }

    pub fn is_loaded(&self, plugin_id: &str) -> bool {
        self.loaded.contains_key(plugin_id)
    }
}
