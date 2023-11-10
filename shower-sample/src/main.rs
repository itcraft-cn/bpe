use shower::{def_action_with_callback, def_record, new_data, start, stop, Column, U8Bytes};
use std::{
    env, thread,
    time::{Duration, SystemTime},
};

const LOOP_SIZE: usize = 100000;

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
    slice[0] = 1;
    slice[8] = 2;
    slice[16] = 3;
    slice[24] = 4;
    slice[32] = 5;
    slice[40] = 6;
    slice[48] = 7;
    slice[56] = 8;
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
        "cost time: {:?}ms / {:?}ns",
        duration.as_millis(),
        duration.as_nanos()
    );
}
