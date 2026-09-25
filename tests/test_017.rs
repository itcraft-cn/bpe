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
    WHERE demo.a < -1 AND demo.b < -2 AND demo.c < -3 AND demo.d < -4 AND demo.e < -5
    LIMIT -100
    "#;

#[test]
pub fn test() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let sum_store = Arc::new(AtomicI64::new(0));
    let sum_clone = Arc::clone(&sum_store);
    let id = init_func(sum_clone);
    if id == 0 {
        log::warn!("init failed");
    } else {
        exec_with_time_it(move || gen_new_data(id));
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
        let v1 = fetch_i64(ptr);
        let v2 = fetch_i64(ptr.wrapping_add(8));
        let v3 = fetch_i64(ptr.wrapping_add(16));
        let v4 = fetch_i64(ptr.wrapping_add(24));
        log::info!("data: {v1}|{v2}|{v3}|{v4}");
        let _ = v1 + v2 + v3 + v4;
    } else {
        log::warn!("got null data");
    }
}

fn gen_new_data(id: u16) {
    static WALKER: AtomicI64 = AtomicI64::new(0);
    let u8data = gen_u8_bytes(id, WALKER.fetch_add(1, Ordering::SeqCst));
    let ret = new_data(&u8data);
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
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_long("d"),
            Column::new_long("e"),
            Column::new_long("f"),
            Column::new_long("g"),
            Column::new_long("h"),
            Column::new_long("i"),
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
fn gen_u8_bytes(id: u16, walker: i64) -> U8Bytes {
    let mut u8array = [0_u8; 512];
    let slice = u8array.as_mut_slice();
    let now = now();
    fill_i64(&mut slice[0..8], -walker + 1);
    fill_i64(&mut slice[8..16], -walker + 2);
    fill_i64(&mut slice[16..24], -walker + 3);
    fill_i64(&mut slice[24..32], -walker + 4);
    fill_i64(&mut slice[32..40], -walker + 5);
    fill_i64(&mut slice[40..48], now as i64);
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
pub(crate) fn fill_i64(slice: &mut [u8], data: i64) {
    let p_val = ptr::addr_of!(*slice);
    let p_i64 = p_val as *mut i64;
    unsafe { *p_i64 = data };
}

#[inline]
pub(crate) fn fetch_i64(p_val: *const u8) -> i64 {
    let p_i64 = p_val as *const i64;
    unsafe { *p_i64 }
}
