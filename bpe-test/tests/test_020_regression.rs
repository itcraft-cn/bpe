use bpe::{
    def_aggregate, def_incoming, def_mapper, def_mapper_bind_aggregate, def_stream, new_data,
    start, stop, Column, U8Bytes,
};
use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc, Mutex,
};

/// The engine is a single-threaded process-level singleton, so all tests in this
/// file must run serially to avoid data races on the global state.
/// Use `into_inner` so one failing test does not poison the lock for the rest.
static ENGINE_LOCK: Mutex<()> = Mutex::new(());

fn lock_engine() -> std::sync::MutexGuard<'static, ()> {
    ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Regression tests for defects found in full-analysis 20260815-001:
/// F1 window collapse after ring-buffer wrap, F2/F3 aggregate window semantics,
/// F4 aggregate storage offsets, F5 multiple mappers, F6 real aggregate ids,
/// F7 aggregate input resolution, F8 unknown WHERE column, F9 oversized data_len,
/// F10 division by zero.
///
/// Each test uses distinct record names because the engine is a process-level
/// singleton (start() runs once) and records persist across tests.

fn send_record(id: u16, a: i64) -> bool {
    let mut bytes = [0_u8; 512];
    unsafe { *(bytes.as_mut_ptr() as *mut i64) = a; }
    new_data(&U8Bytes::new_from_vec(id, 512, Vec::from(bytes)))
}

/// F1: the window scan must keep returning up to LIMIT records after the
/// ring buffer wraps (previously collapsed to 1 record after 2048 writes).
#[test]
fn test_window_after_wrap() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo1", vec![Column::new_long("a")]).unwrap();
    let last_size = Arc::new(AtomicUsize::new(0));
    let ls = last_size.clone();
    def_mapper(
        "SELECT reg_demo1.a FROM reg_demo1 WHERE reg_demo1.a >= 1 LIMIT 10",
        move |p| {
            ls.store(p.size(), Ordering::SeqCst);
        },
    )
    .unwrap();
    for v in 1..=3000i64 {
        assert!(send_record(in_id, v), "record {v} should be accepted");
    }
    assert_eq!(
        last_size.load(Ordering::SeqCst),
        10,
        "window scan must return 10 records after ring-buffer wrap"
    );
    stop();
}

/// F2/F3/F4/F6/F7: aggregate is computed over the current window per call with
/// deterministic initial values; results are stored at dense offsets.
/// a=1,2,3 -> sum=6, count=3, avg=2, max=3, min=1 (last callback wins).
#[test]
fn test_aggregate_window_semantics() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo2", vec![Column::new_long("a")]).unwrap();
    def_stream(
        "reg_stream2",
        vec![
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_long("d"),
            Column::new_long("e"),
        ],
    )
    .unwrap();
    let total = Arc::new(AtomicI64::new(0));
    let t2 = total.clone();
    let agg_id = def_aggregate(
        "select _suml(reg_stream2.a), _count(reg_stream2.a), _avg(reg_stream2.a), _maxl(reg_stream2.a), _minl(reg_stream2.a) from reg_stream2",
        move |p| {
            let ptr = p.u8_ptr();
            let s: i64 = unsafe { *(ptr as *const i64) };
            let c: i64 = unsafe { *(ptr.add(8) as *const i64) };
            let a: f64 = unsafe { *(ptr.add(16) as *const f64) };
            let mx: i64 = unsafe { *(ptr.add(24) as *const i64) };
            let mn: i64 = unsafe { *(ptr.add(32) as *const i64) };
            t2.store(s + c + (a as i64) + mx + mn, Ordering::SeqCst);
            log::info!("agg: sum={s} count={c} avg={a} max={mx} min={mn}");
        },
    )
    .unwrap();
    def_mapper_bind_aggregate(
        "SELECT reg_demo2.a FROM reg_demo2 WHERE reg_demo2.a >= 1 LIMIT 10",
        agg_id,
    )
    .unwrap();
    for v in [1i64, 2, 3] {
        assert!(send_record(in_id, v));
    }
    // expected: sum(6) + count(3) + avg(2) + max(3) + min(1) = 15
    assert_eq!(total.load(Ordering::SeqCst), 15, "aggregate window semantics wrong");
    stop();
}

/// F7: aggregate input must be resolved by field NAME against the mapper output.
/// Mapper selects [f, a]; aggregate _suml(stream.a) must read column a, not f.
#[test]
fn test_aggregate_field_name_resolution() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming(
        "reg_demo3",
        vec![
            Column::new_long("f"),
            Column::new_long("a"),
            Column::new_long("b"),
        ],
    )
    .unwrap();
    def_stream(
        "reg_stream3",
        vec![Column::new_long("a"), Column::new_long("b")],
    )
    .unwrap();
    let sum = Arc::new(AtomicI64::new(0));
    let s2 = sum.clone();
    let agg_id = def_aggregate(
        "select _suml(reg_stream3.a) from reg_stream3",
        move |p| {
            let ptr = p.u8_ptr();
            let v: i64 = unsafe { *(ptr as *const i64) };
            s2.store(v, Ordering::SeqCst);
        },
    )
    .unwrap();
    // select f first (offset 0), a second (offset 8); aggregate must read offset 8
    def_mapper_bind_aggregate(
        "SELECT reg_demo3.f, reg_demo3.a FROM reg_demo3 WHERE reg_demo3.a >= 1 LIMIT 10",
        agg_id,
    )
    .unwrap();
    for v in [1i64, 2, 3] {
        let mut bytes = [0_u8; 512];
        unsafe {
            *(bytes.as_mut_ptr() as *mut i64) = v * 100; // f = 100,200,300
            *(bytes.as_mut_ptr().add(8) as *mut i64) = v; // a = 1,2,3
        }
        new_data(&U8Bytes::new_from_vec(in_id, 512, Vec::from(bytes)));
    }
    // sum of a (1+2+3) = 6, NOT sum of f (100+200+300) = 600
    assert_eq!(sum.load(Ordering::SeqCst), 6, "aggregate must read field a by name");
    stop();
}

/// F5: multiple mappers on the same record must all be invoked.
/// Note: the engine contract is that every mapper callback fires on every new_data,
/// with p.size() indicating how many records matched the filter (0 = none).
#[test]
fn test_multiple_mappers() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo4", vec![Column::new_long("a")]).unwrap();
    let s1 = Arc::new(AtomicUsize::new(usize::MAX));
    let s2 = Arc::new(AtomicUsize::new(usize::MAX));
    let x1 = s1.clone();
    let x2 = s2.clone();
    def_mapper(
        "SELECT reg_demo4.a FROM reg_demo4 WHERE reg_demo4.a > 0 LIMIT 10",
        move |p| {
            x1.store(p.size(), Ordering::SeqCst);
        },
    )
    .unwrap();
    def_mapper(
        "SELECT reg_demo4.a FROM reg_demo4 WHERE reg_demo4.a > 100 LIMIT 10",
        move |p| {
            x2.store(p.size(), Ordering::SeqCst);
        },
    )
    .unwrap();
    assert!(send_record(in_id, 50));
    assert_eq!(s1.load(Ordering::SeqCst), 1, "mapper1 (a>0) must match a=50");
    assert_eq!(s2.load(Ordering::SeqCst), 0, "mapper2 (a>100) must not match a=50");
    assert!(send_record(in_id, 200));
    assert_eq!(s1.load(Ordering::SeqCst), 2, "mapper1 must match both");
    assert_eq!(s2.load(Ordering::SeqCst), 1, "mapper2 must match a=200");
    stop();
}

/// F6: multiple aggregates must each be bindable via their real id.
#[test]
fn test_multiple_aggregates() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo5", vec![Column::new_long("a")]).unwrap();
    def_stream("reg_stream5", vec![Column::new_long("a")]).unwrap();
    let s1 = Arc::new(AtomicI64::new(0));
    let s2 = Arc::new(AtomicI64::new(0));
    let a1 = s1.clone();
    let a2 = s2.clone();
    let agg1 = def_aggregate(
        "select _suml(reg_stream5.a) from reg_stream5",
        move |p| {
            let v: i64 = unsafe { *(p.u8_ptr() as *const i64) };
            a1.store(v, Ordering::SeqCst);
        },
    )
    .unwrap();
    let agg2 = def_aggregate(
        "select _maxl(reg_stream5.a) from reg_stream5",
        move |p| {
            let v: i64 = unsafe { *(p.u8_ptr() as *const i64) };
            a2.store(v, Ordering::SeqCst);
        },
    )
    .unwrap();
    assert_ne!(agg1, agg2, "two aggregates must have distinct ids");
    def_mapper_bind_aggregate(
        "SELECT reg_demo5.a FROM reg_demo5 WHERE reg_demo5.a >= 1 LIMIT 10",
        agg1,
    )
    .unwrap();
    def_mapper_bind_aggregate(
        "SELECT reg_demo5.a FROM reg_demo5 WHERE reg_demo5.a >= 1 LIMIT 10",
        agg2,
    )
    .unwrap();
    for v in [1i64, 2, 3] {
        assert!(send_record(in_id, v));
    }
    assert_eq!(s1.load(Ordering::SeqCst), 6, "agg1 sum must be 6");
    assert_eq!(s2.load(Ordering::SeqCst), 3, "agg2 max must be 3");
    stop();
}

/// F8: unknown column in WHERE must return None, not panic.
#[test]
fn test_unknown_where_column() {
    let _g = lock_engine();
    start();
    def_incoming("reg_demo6", vec![Column::new_long("a")]).unwrap();
    let rs = def_mapper(
        "SELECT reg_demo6.a FROM reg_demo6 WHERE reg_demo6.nonexistent > 1 LIMIT 10",
        |_| {},
    );
    assert!(rs.is_none(), "unknown WHERE column must fail definition, not panic");
    stop();
}

/// F9: payload larger than record_size must be rejected, not corrupt the buffer.
#[test]
fn test_oversized_data_len() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo7", vec![Column::new_long("a")]).unwrap();
    let big = vec![0_u8; 4096]; // > record_size (512)
    let ret = new_data(&U8Bytes::new_from_vec(in_id, big.len(), big));
    assert!(!ret, "data_len > record_size must be rejected");
    stop();
}

/// F10: division/modulo by zero in SELECT must not panic.
#[test]
fn test_div_zero() {
    let _g = lock_engine();
    start();
    let in_id = def_incoming("reg_demo8", vec![Column::new_long("a")]).unwrap();
    def_mapper(
        "SELECT _div(reg_demo8.a, 0), _mod(reg_demo8.a, 0) FROM reg_demo8 WHERE reg_demo8.a >= 1 LIMIT 10",
        |_| {},
    )
    .unwrap();
    assert!(send_record(in_id, 7), "div by zero must not crash");
    stop();
}
