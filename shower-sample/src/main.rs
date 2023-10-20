use log::*;
use shower::{def_action, new_data, start, stop, U8Bytes};
use std::{
    env, thread,
    time::{Duration, SystemTime},
};

const LOOP_SIZE: usize = 10000000;

const SQL: &str = r#"
    SELECT _1.__1, _1.__2, _1.__3, _sub(_add(_1.__4, _1.__4), _1.__5)
    FROM _1
    WHERE (_1.__1 = 1 AND _1.__2 = 2) OR (_1.__1 = 3 AND _1.__2 = 4)
    LIMIT 10
    "#;

pub fn main() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action(SQL);
    exec_with_time_it(gen_new_data);
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    info!("thread:{} started", thread::current().name().unwrap());
    let mut u8array = [0u8; 512];
    let slice = u8array.as_mut_slice();
    for i in 0..8 {
        let data = i + 1;
        fill_u64(&mut slice[i * 8..data * 8], data as u64);
    }
    let mut u64array = [0u64; 8];
    for i in 0..8 {
        u64array[i] = fetch_u64(&slice[i * 8..(i + 1) * 8]);
    }
    info!("data:{:?}", u64array);
    let u8data = U8Bytes::new_from_vec(1, 512, Vec::from(u8array));
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
    }
}

#[inline]
fn fill_u64(slice: &mut [u8], data: u64) {
    slice[0] = data as u8;
    slice[1] = (data >> 1) as u8;
    slice[2] = (data >> 2) as u8;
    slice[3] = (data >> 3) as u8;
    slice[4] = (data >> 4) as u8;
    slice[5] = (data >> 5) as u8;
    slice[6] = (data >> 6) as u8;
    slice[7] = (data >> 7) as u8;
}

#[inline]
fn fetch_u64(slice: &[u8]) -> u64 {
    (slice[0] as u64)
        | ((slice[1] as u64) << 1)
        | ((slice[2] as u64) << 2)
        | ((slice[3] as u64) << 3)
        | ((slice[4] as u64) << 4)
        | ((slice[5] as u64) << 5)
        | ((slice[6] as u64) << 6)
        | ((slice[7] as u64) << 7)
}

pub fn exec_with_time_it<F>(f: F)
where
    F: Fn(),
{
    let start = SystemTime::now();
    f();
    let end = SystemTime::now();
    let duration = end
        .duration_since(start)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    log::info!("cost time: {:?}ms", duration.as_millis());
    log::info!("cost time: {:?}ns", duration.as_nanos());
}
