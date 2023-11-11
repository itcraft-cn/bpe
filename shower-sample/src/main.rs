use shower::{def_action_with_callback, def_record, new_data, start, stop, Column, U8Bytes};
use std::{
    env, ptr, thread,
    time::{Duration, SystemTime},
};

const LOOP_SIZE: usize = 1000000;

const SQL: &str = r#"
    SELECT _1.__1, _1.__2, _1.__3, _sub(_add(_1.__4, _1.__4), _1.__5)
    FROM _1
    WHERE (_1.__1 = 1 AND _1.__2 = 2) OR (_1.__1 = 3 AND _1.__2 = 4)
    LIMIT 10
    "#;

pub fn main() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action_with_callback(SQL, |_vec| {});
    exec_with_time_it(gen_new_data);
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    let id = def_record(vec![
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_string(448),
    ]);
    log::info!("assigned id:{}", id);
    let u8data = gen_u8_bytes(id);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            log::debug!("send success");
        } else {
            log::warn!("send failed");
        }
    }
}

fn gen_u8_bytes(id: u16) -> U8Bytes {
    let mut u8array = [0u8; 512];
    let slice = u8array.as_mut_slice();
    fill_u64(&mut slice[0..8], 1);
    fill_u64(&mut slice[8..16], 2);
    fill_u64(&mut slice[16..24], 3);
    fill_u64(&mut slice[24..32], 4);
    fill_u64(&mut slice[32..40], 5);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
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
    log::info!(
        "cost time: {:?}ms / {:?}ns, use {:?}ns per operation",
        duration.as_millis(),
        duration.as_nanos(),
        1f64 * (duration.as_nanos() as f64) / (LOOP_SIZE as f64)
    );
}

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *mut u64;
    unsafe { *p_u64 = data };
}
