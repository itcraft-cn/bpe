use crate::consts::*;
use config::Config;
use hashbrown::HashMap;
use state::Storage;
use std::{env, sync::Once};

pub(crate) static CONFIG_STORE: Storage<ShowerConfig> = Storage::new();

pub(crate) struct ShowerConfig {
    pub(crate) map: HashMap<String, String>,
}

impl ShowerConfig {

    pub(crate) fn fetch_cfg_str(&self, key: &str) -> String {
        self.map.get(key).unwrap().to_owned()
    }

    pub(crate) fn fetch_cfg_bool(&self, key: &str) -> bool {
        "true".eq_ignore_ascii_case(self.fetch_cfg_str(key).as_str())
    }

    pub(crate) fn _fetch_cfg_usize(&self, key: &str) -> usize {
        usize::from_str_radix(self.fetch_cfg_str(key).as_str(), 10).unwrap()
    }
}


pub(crate) fn load_config() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let cfg_file = compose_file_name_with_base_dir(SHOWER_FILENAME_CONFIG_TOML);
        let cfg_map = Config::builder()
            .add_source(config::File::with_name(cfg_file.as_str()))
            .build()
            .unwrap()
            .try_deserialize::<std::collections::HashMap<String, String>>()
            .unwrap();
        let mut map = hashbrown::HashMap::new();
        for iter in cfg_map.iter() {
            map.insert(iter.0.clone(), iter.1.clone());
        }
        CONFIG_STORE.set(ShowerConfig { map });
    });
}

pub(crate) fn compose_file_name_with_base_dir(filename: &str) -> String {
    let mut full_filename = fetch_base_dir();
    full_filename.push('/');
    full_filename.push_str(filename);
    full_filename
}

fn fetch_base_dir() -> String {
    let rs = env::var(SHOWER_ENV_HOME_KEY);
    if let Ok(path) = rs {
        path
    } else {
        String::from(".")
    }
}

pub(crate) fn get_config() -> &'static ShowerConfig {
    let cfg = CONFIG_STORE.get();
    cfg
}
