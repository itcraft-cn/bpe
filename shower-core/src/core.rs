use crate::{
    cfg::{get_config, load_config},
    consts::{KEY_ONCE_FETCH_RANGE, KEY_QUEUE_CAPACITY, KEY_STORED_TICK_SIZE},
    data::{U8Tick, DEEP_TICK_SIZE, TICK_SIZE},
    logger::init_logger,
    store, DeepTick, Tick,
};
use log::*;
use multiqueue::{mpmc_queue, MPMCReceiver, MPMCSender};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Once, PoisonError, RwLock,
    },
    thread::{self, JoinHandle},
};

const ONE_SEC: u64 = 1000;

static mut ACTIVE: RwLock<AtomicBool> = RwLock::new(AtomicBool::new(true));
static mut OPT_SENDER: Option<MPMCSender<U8Tick>> = None;
static mut OPT_RECV_TH: Option<JoinHandle<()>> = None;

static mut STORED_TICK_SIZE: usize = 0;

static mut TICK_VEC_SIZE: usize = 0;
static mut DEEP_TICK_VEC_SIZE: usize = 0;

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
    let result = AtomicBool::new(false);

    let capacity = get_config().fetch_cfg_usize(KEY_QUEUE_CAPACITY);

    let (sender, receiver) = mpmc_queue(capacity as u64);

    unsafe {
        OPT_SENDER.get_or_insert(sender);
        STORED_TICK_SIZE = get_config().fetch_cfg_usize(KEY_STORED_TICK_SIZE);
        TICK_VEC_SIZE = TICK_SIZE * STORED_TICK_SIZE;
        DEEP_TICK_VEC_SIZE = DEEP_TICK_SIZE * STORED_TICK_SIZE;
    }

    let rs = thread::Builder::new()
        .name(String::from("receiver"))
        .spawn(move || handle_recv(receiver));

    if let Ok(recv_th) = rs {
        unsafe {
            OPT_RECV_TH.get_or_insert(recv_th);
        }
        result.store(true, Ordering::SeqCst);
    } else {
        result.store(false, Ordering::SeqCst);
    }

    result.load(Ordering::SeqCst)
}

fn handle_recv(receiver: MPMCReceiver<U8Tick>) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[0]);
    let cfg = get_config();
    let walker = AtomicU64::new(0);
    let once_fetch_range = cfg.fetch_cfg_usize(KEY_ONCE_FETCH_RANGE);
    loop {
        let iter = receiver.try_iter();
        let mut count = 0;
        iter.take(once_fetch_range).for_each(|tick| {
            process_tick(tick);
            count += 1;
        });
        walker.fetch_add(count, Ordering::SeqCst);
        if !check_active() {
            let now = walker.load(Ordering::SeqCst);
            info!("finally, received {} ticks", now);
            break;
        }
    }
}

fn process_tick(tick: U8Tick) {
    store::insert(tick);
}

fn check_active() -> bool {
    unsafe {
        ACTIVE
            .get_mut()
            .unwrap_or_else(handle_fetch_error_ret_fake)
            .load(Ordering::SeqCst)
    }
}

fn handle_fetch_error_ret_fake(e: PoisonError<&mut AtomicBool>) -> &mut AtomicBool {
    static mut FAKE: AtomicBool = AtomicBool::new(false);
    warn!("Failed to read ACTIVE: {:?}", e);
    unsafe { &mut FAKE }
}

pub fn stop() {
    static STOP: Once = Once::new();
    STOP.call_once(actual_stop);
}

fn actual_stop() {
    unsafe {
        ACTIVE
            .get_mut()
            .unwrap_or_else(handle_fetch_error)
            .store(false, Ordering::SeqCst);
    }
    info!("mark as deactived");
    unsafe {
        wait_receiver_quit();
    }
}

unsafe fn wait_receiver_quit() {
    loop {
        if OPT_RECV_TH.as_ref().unwrap().is_finished() {
            info!("receiver thread is finished");
            break;
        }
        info!("receiver thread is running, waiting for it");
        thread::sleep(std::time::Duration::from_millis(ONE_SEC));
    }
}

fn handle_fetch_error(e: PoisonError<&mut AtomicBool>) -> &mut AtomicBool {
    warn_and_panic!("fail to set active to false, cannot stop threads:{}", e);
}

pub fn new_tick_data(data: Tick) -> bool {
    unsafe { process_data(data.convert(TICK_VEC_SIZE)) }
}

pub fn new_deep_tick_data(data: DeepTick) -> bool {
    unsafe { process_data(data.convert(DEEP_TICK_VEC_SIZE)) }
}

unsafe fn process_data(data: U8Tick) -> bool {
    if let Some(sender) = OPT_SENDER.as_ref() {
        let rs = sender.try_send(data);
        if let Ok(_res) = rs {
            true
        } else {
            let err = rs.err().unwrap();
            warn!(
                "Failed to send data to queue, dropped. reseason: [{:?}-->{}]",
                &err, &err
            );
            false
        }
    } else {
        false
    }
}

pub fn def_action(js: &str) {
    info!("{}", js);
}
