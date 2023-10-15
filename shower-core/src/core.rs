use crate::{
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::U8Bytes,
    logger::init_logger,
    store,
};
use log::*;
use std::sync::Once;

static mut DEBUG: bool = false;

pub fn start() -> bool {
    static START: Once = Once::new();
    let mut opt = None;
    START.call_once(|| {
        opt.replace(actual_start());
    });
    if opt.is_none() {
        debug!("already started, skipping");
        true
    } else {
        opt.unwrap_or(false)
    }
}

fn actual_start() -> bool {
    load_config();
    init_logger(get_config());

    let cfg = get_config();

    unsafe {
        DEBUG = cfg.fetch_cfg_bool(KEY_DEV_MODE);
    }

    true
}

fn process_data(data: &U8Bytes) {
    store::insert(&data);
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

fn actual_stop() {
    info!("mark as deactived");
}

pub fn new_data(data: &U8Bytes) -> bool {
    process_data(&data);
    true
}

pub fn def_action(js: &str) {
    info!("{}", js);
}
