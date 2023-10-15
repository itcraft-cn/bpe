mod test_log;
mod test_timestamp;

use log::*;
use shower::{new_data, start, stop, U8Bytes};
use std::{env, thread};
use test_log::init_logger;
use test_timestamp::special_timestamp;

const TM_TEST_SIZE: usize = 10000;
const LOOP_SIZE: usize = 100000000;

#[test]
fn test_timestamp() {
    init_logger();
    let timestamp = special_timestamp(2023, 10, 12, 12, 46, 3);
    for i in 0..TM_TEST_SIZE {
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
    let u8data = U8Bytes::new_from_vec(16, 288, vec![0u8; 288]);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
    }
}
