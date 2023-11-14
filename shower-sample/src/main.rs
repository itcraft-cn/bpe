use shower::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{
    env, ptr, thread,
    time::{Duration, SystemTime},
};

const LOOP_SIZE: usize = 100000;

const SQL: &str = r#"
    SELECT demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
    FROM demo
    WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
    LIMIT 10
    "#;

pub fn main() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    exec_with_time_it(gen_new_data);
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    if let Some(id) = def_incoming(
        "demo",
        vec![
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_long("d"),
            Column::new_long("e"),
            Column::new_long("f"),
            Column::new_long("g"),
            Column::new_long("h"),
            Column::new_string("i", 448),
        ],
    ) {
        log::info!("assigned id:{}", id);
        let u8data = gen_u8_bytes(id);
        def_mapper(SQL, |_vec| {});
        for _ in 1..=LOOP_SIZE {
            let ret = new_data(&u8data);
            if ret {
                log::debug!("send success");
            } else {
                log::warn!("send failed");
            }
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
