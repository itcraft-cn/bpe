use crate::{cfg::get_config, consts::SHOWER_ENV_HOME_KEY, new_data, start, stop, Tick};
use chrono::NaiveDate;
use log::*;
use std::{env, thread, time::Duration};

#[test]
fn test_new_proc() {
    env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
    start();
    let rs = thread::Builder::new()
        .name(String::from("caller"))
        .spawn(move || gen_new_data());
    if rs.is_ok() {
        let th = rs.unwrap();
        let _ = th.join();
        thread::sleep(Duration::from_secs(1));
        stop();
    }
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    info!("thread:{} started", thread::current().name().unwrap());
    let loop_size: usize = get_config().fetch_cfg_usize("test_loop_size");
    let range_size: usize = get_config().fetch_cfg_usize("test_range_size");
    let mut timestamp = special_timestamp(2023, 7, 10, 9, 59, 59);
    for i in 1..=loop_size {
        let v = i as u64;
        let ret = new_data(Tick::new((i % 32) as u16, v, v, v, v, timestamp));
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
        if i % range_size == 0 {
            timestamp += 1000;
            info!("sending {} ticks", i);
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn special_timestamp(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> u64 {
    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default();
    let date_time = date.and_hms_opt(hour, min, sec).unwrap_or_default();
    date_time.timestamp_millis() as u64
}
