use crate::consts::{BPE_ENV_HOME_KEY, BPE_FILENAME_CONFIG_TOML};
use config::{Config, File};
use hashbrown::HashMap;
use std::{env, sync::OnceLock};

pub(crate) static WRAPPED_CONFIG: OnceLock<BpeConfig> = OnceLock::new();

pub(crate) struct BpeConfig {
    pub(crate) map: HashMap<String, String>,
}

impl BpeConfig {
    pub(crate) fn fetch_cfg_str(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    pub(crate) fn fetch_cfg_bool(&self, key: &str) -> bool {
        if let Some(val) = self.fetch_cfg_str(key) {
            "true".eq_ignore_ascii_case(val.as_str())
        } else {
            false
        }
    }

    pub(crate) fn fetch_cfg_usize(&self, key: &str) -> Option<usize> {
        if let Some(val) = self.fetch_cfg_str(key) {
            val.as_str().parse::<usize>().ok()
        } else {
            None
        }
    }
}

pub(crate) fn load_config() {
    WRAPPED_CONFIG.get_or_init(|| {
        let mut map = HashMap::new();
        let cfg_file = compose_file_name_with_base_dir(BPE_FILENAME_CONFIG_TOML);
        if let Ok(config) = Config::builder()
            .add_source(File::with_name(cfg_file.as_str()))
            .build()
        {
            if let Ok(cfg_map) =
                config.try_deserialize::<std::collections::HashMap<String, String>>()
            {
                for iter in cfg_map.iter() {
                    map.insert(iter.0.clone(), iter.1.clone());
                }
            }
        }
        BpeConfig { map }
    });
}

pub(crate) fn compose_file_name_with_base_dir(filename: &str) -> String {
    let mut full_filename = fetch_base_dir();
    full_filename.push('/');
    full_filename.push_str(filename);
    full_filename
}

fn fetch_base_dir() -> String {
    if let Ok(path) = env::var(BPE_ENV_HOME_KEY) {
        path
    } else if let Ok(dir) = env::current_dir() {
        format!("{dir:?}")
    } else {
        String::from(".")
    }
}

pub(crate) fn get_config() -> &'static BpeConfig {
    let cfg = WRAPPED_CONFIG.get();
    cfg.unwrap()
}
