use crate::{
    cfg::{get_config, load_config},
    consts::{KEY_DEEP_TICK_DEPTH, KEY_DEV_MODE, KEY_STORED_TICK_SIZE},
    data::{TickConvU8, U8Tick, DEEP_TICK_SIZE, TICK_SIZE},
    logger::init_logger,
    store_bar, store_tick, DeepTick, Tick,
};
use log::*;
use std::sync::Once;

static mut DEBUG: bool = false;

static mut STORED_TICK_SIZE: usize = 0;

static mut TICK_VEC_SIZE: usize = 0;
static mut DEEP_TICK_VEC_SIZE: usize = 0;

static mut DEEP_TICK_DEPTH: usize = 0;

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
        STORED_TICK_SIZE = cfg.fetch_cfg_usize(KEY_STORED_TICK_SIZE);
        TICK_VEC_SIZE = TICK_SIZE * STORED_TICK_SIZE;
        DEEP_TICK_VEC_SIZE = DEEP_TICK_SIZE * STORED_TICK_SIZE;
        DEEP_TICK_DEPTH = cfg.fetch_cfg_usize(KEY_DEEP_TICK_DEPTH);
    }

    true
}

fn process_event(event: Event) {
    match event {
        Event::NewTick(tick) => {
            process_tick(&tick, unsafe { TICK_VEC_SIZE }, 2);
        }
        Event::NewDeepTick(tick) => {
            process_tick(&tick, unsafe { DEEP_TICK_VEC_SIZE }, 59);
        }
        Event::ComputeBar(u8tick, idx) => {
            store_bar::update_tick(&u8tick, idx);
        }
    }
}

fn process_tick(tick: &dyn TickConvU8, size: usize, idx: usize) {
    let u8tick = tick.convert(size);
    store_tick::insert(&u8tick);
    process_event(Event::ComputeBar(u8tick, idx));
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

fn actual_stop() {
    info!("mark as deactived");
}

pub fn new_tick_data(data: Tick) -> bool {
    process_event(Event::NewTick(data));
    true
}

pub fn new_deep_tick_data(data: DeepTick) -> bool {
    process_event(Event::NewDeepTick(data));
    true
}

pub fn def_action(js: &str) {
    info!("{}", js);
}

enum Event {
    NewTick(Tick),
    NewDeepTick(DeepTick),
    ComputeBar(U8Tick, usize),
}
