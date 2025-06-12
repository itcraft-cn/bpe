use bbpe::{
    def_aggregate, def_incoming, def_mapper_bind_aggregate, def_stream, new_data, start, stop,
    Column, U8Bytes,
};
use std::{
    env, ptr,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime},
};

const BASE: f64 = 1000000f64;
const LOOP_SIZE: usize = 1000;

const FILTER_SQL: &str = r#"
    SELECT demo.f, demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
    FROM demo
    WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
    LIMIT 10
    "#;
const AGGREGATE_SQL: &str = r#"
    select _maxd(stream.a), _mind(stream.a), _avg(stream.a), 1
    from stream
    "#;

#[test]
pub fn test() {
    env::set_var("BAMBOOTUBE_HOME", env::current_dir().unwrap());
    start();
    let sum_store = Arc::new(AtomicI64::new(0));
    let sum_clone = Arc::clone(&sum_store);
    let (id1, id2) = init_func(sum_clone);
    if id1 == 0 || id2 == 0 {
        log::warn!("init failed");
    } else {
        let mut u8data = gen_u8_bytes(id1);
        exec_with_time_it(move || gen_new_data(&mut u8data));
        let sum = sum_store.load(Ordering::SeqCst);
        log::info!(
            "sum: {}s({}ns), latency: {}s({}ns)",
            sum as f64 / BASE / 1_000_000_000f64,
            sum,
            (sum as f64 / BASE / LOOP_SIZE as f64) / 1_000_000_000f64,
            sum as f64 / BASE / LOOP_SIZE as f64
        );
    }
    stop();
}

fn init_func(sum_store: Arc<AtomicI64>) -> (u16, u16) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    if let Some((id1, id2)) = define_records() {
        log::info!("define record: {id1}/{id2}");
        if let Some(aggregate_id) = def_aggregate(
            AGGREGATE_SQL,
            #[inline]
            move |params| {
                func_callback(&sum_store, params.u8_ptr(), params.size());
            },
        ) {
            log::info!("define aggregate: {aggregate_id}");
            if let Some(mapper_id) = def_mapper_bind_aggregate(FILTER_SQL, aggregate_id) {
                log::info!("define mapper: {mapper_id}");
                return (id1, id2);
            } else {
                log::warn!("def_mapper_bind_aggregate failed");
            }
        } else {
            log::warn!("def_aggregate failed");
        }
    }
    (0, 0)
}

#[inline]
fn func_callback(sum_store: &Arc<AtomicI64>, data: *const u8, size: usize) {
    let now = now();
    let timestamp = fetch_f64(data);
    let timestamp_min = fetch_f64(data.wrapping_add(8));
    let timestamp_sum = fetch_f64(data.wrapping_add(16));
    let const_val = fetch_i64(data.wrapping_add(24));
    log::info!("data_ptr:{data:?}, size: {size}");
    log::info!("now: {now}, timestamp: {timestamp}");
    log::info!("{timestamp}/{timestamp_min}/{timestamp_sum}/{const_val}");
    let delta = now as f64 - timestamp;
    log::info!("delta: {delta}");
    sum_store.fetch_add((BASE * delta) as i64, Ordering::SeqCst);
}

fn gen_new_data(u8data: &mut U8Bytes) {
    update_now(u8data);
    let ret = new_data(u8data);
    if ret {
        log::debug!("send success");
    } else {
        log::warn!("send failed");
    }
}

fn define_records() -> Option<(u16, u16)> {
    let id1;
    let id2;
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
        id1 = id;
        log::info!("defined incoming: {id1}");
    } else {
        log::warn!("failed to define incoming record");
        return None;
    }
    if let Some(id) = def_stream(
        "stream",
        vec![
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_long("d"),
            Column::new_double("e"),
            Column::new_double("f"),
            Column::new_double("g"),
            Column::new_long("h"),
        ],
    ) {
        id2 = id;
        log::info!("defined stream: {id2}");
    } else {
        log::warn!("failed to define stream record");
        return None;
    }
    Some((id1, id2))
}

#[inline]
fn gen_u8_bytes(id: u16) -> U8Bytes {
    let mut u8array = [0_u8; 512];
    let slice = u8array.as_mut_slice();
    let now = now();
    //log::info!("now: {now}");
    fill_u64(&mut slice[0..8], 1);
    fill_u64(&mut slice[8..16], 2);
    fill_u64(&mut slice[16..24], 3);
    fill_u64(&mut slice[24..32], 4);
    fill_u64(&mut slice[32..40], 5);
    fill_u64(&mut slice[40..48], now);
    //let v = fetch_u64(slice.as_ptr());
    //log::info!("v: {v}");
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}

#[inline]
fn update_now(u8data: &mut U8Bytes) {
    let now = now();
    fill_u64(&mut u8data.bytes_mut()[40..48], now);
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
    for _ in 1..=LOOP_SIZE {
        f();
    }
    let end = SystemTime::now();
    let duration = end
        .duration_since(start)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    log::info!(
        "cost time: {:?}ms / {:?}ns, use {:?}ns per operation",
        duration.as_millis(),
        duration.as_nanos(),
        1_f64 * (duration.as_nanos() as f64) / (LOOP_SIZE as f64)
    );
}

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *mut u64;
    unsafe { *p_u64 = data };
}

#[inline]
pub(crate) fn fetch_i64(p_val: *const u8) -> i64 {
    let p_i64 = p_val as *const i64;
    unsafe { *p_i64 }
}

#[inline]
pub(crate) fn fetch_f64(p_val: *const u8) -> f64 {
    let p_f64 = p_val as *const f64;
    unsafe { *p_f64 }
}
