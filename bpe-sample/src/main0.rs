use bpe::{def_incoming, def_mapper, new_data, start, stop, CallbackParams, Column, U8Bytes};
use std::{
    env, ptr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime},
};

const LOOP_SIZE: usize = 20;
const LOOP_SIZE_F64: f64 = LOOP_SIZE as f64;

const FILTER_SQL: &str = r#"
    SELECT demo.a, demo.b, demo.c, demo.d
    FROM demo
    WHERE demo.a > 1.0 AND demo.b > 2.0 AND demo.c > 3.0 AND demo.d > 4.0 AND demo.e > 5.0
    LIMIT -1
    "#;

pub fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let sum_store = Arc::new(AtomicI64::new(0));
    let sum_clone = Arc::clone(&sum_store);
    let id = init_func(sum_clone);
    if id == 0 {
        log::warn!("init failed");
    } else {
        let u8data = gen_u8_bytes(id);
        exec_with_time_it(move || gen_new_data(&u8data));
        let sum = sum_store.load(Ordering::SeqCst);
        log::info!("sum: {sum}");
    }
    stop();
}

fn init_func(sum_store: Arc<AtomicI64>) -> u16 {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    if let Some(id) = define_record() {
        if let Some(mapper_id) =
            def_mapper(FILTER_SQL, move |params| func_callback(&sum_store, params))
        {
            log::info!("define mapper: {mapper_id}");
            id
        } else {
            log::warn!("def_mapper_bind_aggregate failed");
            0
        }
    } else {
        0
    }
}

#[inline]
fn func_callback(sum_store: &Arc<AtomicI64>, param: CallbackParams) {
    if param.size() > 0 {
        sum_store.fetch_add(1, Ordering::SeqCst);
        let ptr = param.u8_ptr();
        let v1 = fetch_f64(ptr);
        let v2 = fetch_f64(ptr.wrapping_add(8));
        let v3 = fetch_f64(ptr.wrapping_add(16));
        let v4 = fetch_f64(ptr.wrapping_add(24));
        log::info!("data: {v1}|{v2}|{v3}|{v4}");
    } else {
        log::warn!("got null data");
    }
}

fn gen_new_data(u8data: &U8Bytes) {
    let ret = new_data(u8data);
    if ret {
        log::debug!("send success");
    } else {
        log::warn!("send failed");
    }
}

fn define_record() -> Option<u16> {
    if let Some(id) = def_incoming(
        "demo",
        vec![
            Column::new_double("a"),
            Column::new_double("b"),
            Column::new_double("c"),
            Column::new_double("d"),
            Column::new_double("e"),
            Column::new_double("f"),
            Column::new_double("g"),
            Column::new_double("h"),
            Column::new_double("i"),
        ],
    ) {
        log::info!("defined incoming: {id}");
        Some(id)
    } else {
        log::warn!("failed to define incoming record");
        None
    }
}

#[inline]
fn gen_u8_bytes(id: u16) -> U8Bytes {
    let mut u8array = [0_u8; 512];
    let slice = u8array.as_mut_slice();
    let now = now();
    //log::info!("now: {now}");
    fill_f64(&mut slice[0..8], 1.1);
    fill_f64(&mut slice[8..16], 2.2);
    fill_f64(&mut slice[16..24], 3.3);
    fill_f64(&mut slice[24..32], 4.4);
    fill_f64(&mut slice[32..40], 5.5);
    fill_f64(&mut slice[40..48], now as f64 / 500_f64);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}

#[inline]
fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

pub fn exec_with_time_it<F>(mut f: F)
where
    F: FnMut(),
{
    let start = SystemTime::now();
    for _n in 1..=LOOP_SIZE {
        f();
    }
    let end = SystemTime::now();
    let duration = end
        .duration_since(start)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    log::info!("cost time: {duration:?}");
    log::info!("cost time: {:?}ms", duration.as_millis());
    log::info!("cost time: {:?}ns", duration.as_nanos());
    log::info!(
        "cost time: {:?}ns per operation",
        duration.as_nanos() as f64 / LOOP_SIZE_F64
    );
}

#[inline]
pub(crate) fn fill_f64(slice: &mut [u8], data: f64) {
    let p_val = ptr::addr_of!(*slice);
    let p_f64 = p_val as *mut f64;
    unsafe { *p_f64 = data };
}

#[inline]
pub(crate) fn fetch_f64(p_val: *const u8) -> f64 {
    let p_f64 = p_val as *const f64;
    unsafe { *p_f64 }
}
