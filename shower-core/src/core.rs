use crate::{
    action::{call_action, define_action, init_action_store},
    cfg::{get_config, load_config},
    consts::KEY_DEV_MODE,
    data::U8Bytes,
    define::{init_define_store, insert_define, FieldDef},
    ffi::FfiFunc,
    func::FnHolder,
    logger::init_logger,
    store::insert,
};
use log::*;
use std::sync::Once;

static mut DEBUG: bool = false;
static mut ID_STORE: [u8; 8192] = [0; 8192];

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
        init_action_store();
        init_define_store();
    }
    true
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

pub fn def_record(defines: Vec<FieldDef>) -> u16 {
    let id = insert_define(defines);
    let idx = id / 8;
    let bit = id % 8;
    unsafe { ID_STORE[idx as usize] |= 1 << bit };
    id
}

fn actual_stop() {
    info!("mark as deactived");
}

pub fn new_data(data: &U8Bytes) -> bool {
    let id = data.id();
    let idx = id / 8;
    let bit = id % 8;
    if unsafe { ID_STORE[idx as usize] & (1 << bit) == 0 } {
        false
    } else {
        process_data(data);
        true
    }
}

#[inline]
fn process_data(data: &U8Bytes) {
    insert(data);
    call_action(data);
}

pub fn def_action(sql: &str) -> bool {
    define_action(sql, FnHolder::NotExist)
}

pub fn def_action_with_callback<F>(sql: &str, func: F) -> bool
where
    F: Fn(Vec<[u8; 512]>) + Send + 'static,
{
    define_action(sql, FnHolder::Func(Box::new(func)))
}

pub fn def_action_ffi(sql: &str, ffi: Box<dyn FfiFunc>) -> bool {
    define_action(sql, FnHolder::FfiFunc(ffi))
}
