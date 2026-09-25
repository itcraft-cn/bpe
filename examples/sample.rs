// 端到端示例：incoming(demo) -> mapper(filter) -> aggregate(stream)。
// 中等数据量模式：10 万事件，数据均可通过 WHERE 过滤，用于验证
// filter→aggregate 数据链路并顺带观测吞吐，避免日志淹没性能测量。
use bpe::{
    def_aggregate, def_incoming, def_mapper_bind_aggregate, def_stream, new_data, start, stop,
    CallbackParams, Column, U8Bytes,
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

/// 中等数据量：10 万事件，兼顾数据链路验证与轻量吞吐观测。
const LOOP_SIZE: usize = 100_000;
const LOOP_SIZE_F64: f64 = LOOP_SIZE as f64;

const FILTER_SQL: &str = r#"
    SELECT demo.f, demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
    FROM demo
    WHERE demo.a > 1 AND demo.b > 2 AND demo.c > 3 AND demo.d > 4 AND demo.e > 5
    LIMIT -10
    "#;
const AGGREGATE_SQL: &str = r#"
    select _firstl(stream.a), _lastl(stream.a), _minl(stream.a), _maxl(stream.a)
    from stream
    "#;

pub fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let sum_store = Arc::new(AtomicI64::new(0));
    let sum_clone = Arc::clone(&sum_store);
    let (id1, id2) = init_func(sum_clone);
    if id1 == 0 || id2 == 0 {
        log::warn!("init failed");
    } else {
        let mut u8data = gen_u8_bytes(id1);
        let mut seq: i64 = 0;
        exec_with_time_it(move || {
            seq += 1;
            gen_new_data(&mut u8data, seq);
        });
        // sum_store 实际累计的是聚合回调有效结果数（命中数）。
        let hits = sum_store.load(Ordering::SeqCst);
        log::info!("aggregate hits: {hits}");
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
            move |param| {
                func_callback(&sum_store, param);
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
fn func_callback(sum_store: &Arc<AtomicI64>, param: CallbackParams) {
    if param.size() > 0 {
        let n = sum_store.fetch_add(1, Ordering::SeqCst);
        // 中等数据量下按固定间隔打印运行中的聚合值，避免逐事件日志淹没测量。
        if n % 10_000 == 0 {
            let ptr = param.u8_ptr();
            let v1 = fetch_i64(ptr);
            let v2 = fetch_i64(ptr.wrapping_add(8));
            let v3 = fetch_i64(ptr.wrapping_add(16));
            let v4 = fetch_i64(ptr.wrapping_add(24));
            log::info!("data[{n}]: {v1}|{v2}|{v3}|{v4}");
            let _v = v1 + v2 + v3 + v4;
        }
    } else {
        log::debug!("got null data");
    }
}

fn gen_new_data(u8data: &mut U8Bytes, seq: i64) {
    update_now(u8data);
    // demo.a 随事件变化（保持 >1 以通过过滤），用于观测聚合的 first/last/min/max。
    fill_i64(&mut u8data.bytes_mut()[0..8], 2 + seq % 1000);
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
            Column::new_long("e"),
            Column::new_long("f"),
            Column::new_long("g"),
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
    // 取值需满足 WHERE a>1 AND b>2 AND c>3 AND d>4 AND e>5，确保事件可通过过滤。
    fill_i64(&mut slice[0..8], 2);
    fill_i64(&mut slice[8..16], 3);
    fill_i64(&mut slice[16..24], 4);
    fill_i64(&mut slice[24..32], 5);
    fill_i64(&mut slice[32..40], 6);
    fill_i64(&mut slice[40..48], now as i64);
    log::info!("v: {}", now as i64);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}

#[inline]
fn update_now(u8data: &mut U8Bytes) {
    let now = now();
    // log::info!("v: {}", now as i64);
    fill_i64(&mut u8data.bytes_mut()[40..48], now as i64);
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
