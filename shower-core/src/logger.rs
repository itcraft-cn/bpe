use crate::{
    cfg::{compose_file_name_with_base_dir, ShowerConfig},
    consts::{KEY_DEV_MODE, KEY_LOG_DEV_CONFIG_FILE, KEY_LOG_PROD_CONFIG_FILE},
};
use log::info;
use std::sync::Once;

/// 日志初始化，写入 `stdout`，并写入临时文件夹下 `shower.log`
pub(crate) fn init_logger(cfg: &ShowerConfig) {
    static INIT: Once = Once::new();
    INIT.call_once(|| init(cfg));
}

fn init(cfg: &ShowerConfig) {
    if cfg.fetch_cfg_bool(KEY_DEV_MODE) {
        init_log4rs(cfg, KEY_LOG_DEV_CONFIG_FILE);
    } else {
        init_log4rs(cfg, KEY_LOG_PROD_CONFIG_FILE);
    }
    info!("booting up");
}

fn init_log4rs(cfg: &ShowerConfig, cfg_key: &str) {
    let filename = compose_file_name_with_base_dir(cfg.fetch_cfg_str(cfg_key).as_str());
    let rs = log4rs::init_file(filename.as_str(), Default::default());
    if_match_panic!(rs, "failed to initialize logger");
}
