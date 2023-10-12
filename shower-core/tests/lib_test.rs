mod test_log;
mod test_timestamp;

use log::*;
use shower::{new_tick_data, start, stop, Tick};
use std::{env, thread, time::Duration};
use test_log::init_logger;
use test_timestamp::special_timestamp;

const LOOP_SIZE: usize = 50000;
const RANGE_SIZE: usize = 100;
const WAIT_TIME: usize = 1;

#[test]
fn test_timestamp() {
    init_logger();
    let timestamp = special_timestamp(2023, 10, 12, 12, 46, 3);
    for i in 0..LOOP_SIZE {
        log::info!("timestamp: {}", timestamp + i as u64 * 1000);
    }
}

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    gen_new_data();
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    info!("thread:{} started", thread::current().name().unwrap());
    let mut timestamp = special_timestamp(2023, 7, 10, 9, 59, 59);
    for i in 1..=LOOP_SIZE {
        let v = i as u64;
        let ret = new_tick_data(Tick::new((i % 32) as u16, v, v, v, v, timestamp));
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
        if i % RANGE_SIZE == 0 {
            timestamp += 1000;
            info!("sending {} ticks", i);
            thread::sleep(Duration::from_millis(WAIT_TIME as u64));
        }
    }
}
