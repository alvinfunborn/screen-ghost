mod face;
mod monitoring;
mod system;

pub use face::*;
pub use monitoring::*;
pub use system::*;

use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use toml;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Config {
    pub face: Option<FaceConfig>,
    pub monitoring: Option<MonitoringConfig>,
    pub system: Option<SystemConfig>,
}


pub fn get_config_path() -> Option<String> {
    let config_paths = vec!["config.toml", "src-tauri/config.toml", "../config.toml"];
    for path in config_paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

pub fn load_config() -> Config {
    if let Some(path) = get_config_path() {
        let config_str = fs::read_to_string(&path)
            .expect(format!("[load_config] Failed to read config file: {}", path).as_str());
        let config: Config = toml::from_str(&config_str)
            .expect(format!("[load_config] Failed to parse config file: {}", path).as_str());
        info!("[load_config] load config from{} : {:?}", path, config);
        return config;
    }
    panic!("please check the config file: config.toml exists");
}

// 全局配置实例
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub static CONFIG: Lazy<Mutex<Option<Config>>> = Lazy::new(|| Mutex::new(None));

// 初始化配置
pub fn init_config() -> Config {
    let config = load_config();
    let mut config_guard = CONFIG.lock().unwrap();
    *config_guard = Some(config.clone());
    config
}

// 获取配置
pub fn get_config() -> Option<Config> {
    CONFIG.lock().unwrap().clone()
}
