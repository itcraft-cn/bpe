use crate::{consts::SHOWER_ENV_HOME_KEY, new_data, start, stop, Tick};
use log::*;
use std::{env, thread, time::Duration};

const LOOP_SIZE: i32 = 100000000;

#[test]
fn test_new_proc() {
    env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
    start();
    let rs = thread::Builder::new()
        .name(String::from("caller"))
        .spawn(move || gen_new_data());
    if rs.is_ok() {
        let th = rs.unwrap();
        thread::sleep(Duration::from_secs(1));
        let _ = th.join();
        stop();
    }
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    info!("thread:{} started", thread::current().name().unwrap());
    for i in 0..LOOP_SIZE {
        let v = i as u64;
        let ret = new_data(Tick::new(
            7686, v as u128, v as u128, v as u128, v as u128, v,
        ));
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
    }
}
