use bpe::{
    def_dimension, def_keyed_window_aggregate, def_mapper, def_stream, new_data,
    remove_dimension, start, stop, update_dimension, Column, U8Bytes, Window,
};
use std::{
    env,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

static ENGINE_LOCK: Mutex<()> = Mutex::new(());

fn lock_engine() -> std::sync::MutexGuard<'static, ()> {
    ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sends one record with the given column values (offsets 0, 8, 16, ...).
fn send_cols(id: u16, vals: &[i64]) -> bool {
    let mut vec = vec![0_u8; 512];
    let ptr = vec.as_mut_ptr();
    for (i, v) in vals.iter().enumerate() {
        unsafe { *(ptr.add(i * 8) as *mut i64) = *v };
    }
    new_data(&U8Bytes::new_from_vec(id, 512, vec))
}

/// Per-key window: one result row per key, layout [key][field0][field1]...
#[test]
fn test_keyed_window() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let sid = def_stream(
        "k_s1",
        vec![
            Column::new_long("ts"),
            Column::new_long("uid"),
            Column::new_long("amount"),
        ],
    )
    .unwrap();
    // collect (start, end) -> sorted rows (key, count, sum)
    let out: Arc<Mutex<Vec<(i64, i64, Vec<(i64, i64, i64)>)>>> = Arc::new(Mutex::new(vec![]));
    let o2 = out.clone();
    def_keyed_window_aggregate(
        "SELECT _count(k_s1.amount), _suml(k_s1.amount) FROM k_s1 WHERE k_s1.amount > 0",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        "uid",
        0,
        move |p| {
            let mut rows = vec![];
            for i in 0..p.size() {
                let row = unsafe { p.u8_ptr().add(i * p.step()) };
                let key: i64 = unsafe { *(row as *const i64) };
                let cnt: i64 = unsafe { *(row.add(8) as *const i64) };
                let sum: i64 = unsafe { *(row.add(16) as *const i64) };
                rows.push((key, cnt, sum));
            }
            rows.sort();
            o2.lock()
                .unwrap()
                .push((p.window_start_ms(), p.window_end_ms(), rows));
        },
    )
    .unwrap();
    // window [1000,1200): uid=1 x2 (amount 10,5), uid=2 x1 (amount 20)
    assert!(send_cols(sid, &[1000, 1, 10]));
    assert!(send_cols(sid, &[1100, 2, 20]));
    assert!(send_cols(sid, &[1200, 1, 5]));
    thread::sleep(Duration::from_millis(150));

    let out = out.lock().unwrap().clone();
    assert_eq!(out.len(), 2, "two keyed windows, got {out:?}");
    let w1 = out.iter().find(|r| r.0 == 1000).unwrap();
    assert_eq!(w1.2.len(), 2, "window [1000,1200) has two keys");
    assert_eq!(w1.2[0], (1, 1, 10), "uid=1: count=1 sum=10 (ts=1200 belongs to next window)");
    assert_eq!(w1.2[1], (2, 1, 20), "uid=2: count=1 sum=20");
    let w2 = out.iter().find(|r| r.0 == 1200).unwrap();
    assert_eq!(w2.2.len(), 1, "window [1200,1400) has one key");
    assert_eq!(w2.2[0], (1, 1, 5), "uid=1: count=1 sum=5");
    stop();
}

/// Dimension lookup in the WHERE clause (JIT path).
#[test]
fn test_dimension_where() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let dim = def_dimension().unwrap();
    update_dimension(dim, 42, 1);
    let sid = def_stream("d_s1", vec![Column::new_long("id"), Column::new_long("a")]).unwrap();
    let size = Arc::new(Mutex::new(0usize));
    let s2 = size.clone();
    // dim id is process-global and increments across tests, so build the SQL dynamically
    let sql = format!(
        "SELECT d_s1.a FROM d_s1 WHERE _dim_has({dim}, d_s1.id) > 0 LIMIT 10"
    );
    def_mapper(&sql, move |p| {
        *s2.lock().unwrap() = p.size();
    })
    .unwrap();
    assert!(send_cols(sid, &[42, 1]));
    assert!(send_cols(sid, &[7, 2]));
    assert_eq!(*size.lock().unwrap(), 1, "only key 42 passes the blacklist filter");
    // remove 42 from the dimension: no more matches
    remove_dimension(dim, 42);
    assert!(send_cols(sid, &[42, 3]));
    assert_eq!(*size.lock().unwrap(), 0, "after removal, key 42 no longer matches");
    stop();
}

/// Dimension lookup in SELECT fields (interpreter path).
#[test]
fn test_dimension_select() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let dim = def_dimension().unwrap();
    update_dimension(dim, 42, 100);
    let sid = def_stream("d_s2", vec![Column::new_long("id"), Column::new_long("a")]).unwrap();
    let got: Arc<Mutex<Vec<(i64, i64)>>> = Arc::new(Mutex::new(vec![]));
    let g2 = got.clone();
    let sql = format!(
        "SELECT _dim_has({dim}, d_s2.id), _dim_get({dim}, d_s2.id) FROM d_s2 WHERE d_s2.a > 0 LIMIT 10"
    );
    def_mapper(&sql, move |p| {
        let ptr = p.u8_ptr();
        let has: i64 = unsafe { *(ptr as *const i64) };
        let val: i64 = unsafe { *(ptr.add(8) as *const i64) };
        g2.lock().unwrap().push((has, val));
    })
    .unwrap();
    assert!(send_cols(sid, &[42, 1]));
    assert!(send_cols(sid, &[7, 2]));
    let v = got.lock().unwrap().clone();
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], (1, 100), "key 42: has=1 get=100");
    assert_eq!(v[1], (0, 0), "key 7: has=0 get=0");
    stop();
}

/// Watermark drops late (out-of-order) records beyond the lag tolerance.
/// Window [1000,1200) with lag=100: a record at ts=1100 arriving after
/// max_seen=1500 is older than watermark 1400 and must be dropped.
#[test]
fn test_watermark_late_drop() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let sid = def_stream("w_s1", vec![Column::new_long("ts"), Column::new_long("a")]).unwrap();
    let out: Arc<Mutex<Vec<(i64, i64, i64, i64)>>> = Arc::new(Mutex::new(vec![]));
    let o2 = out.clone();
    def_keyed_window_aggregate(
        "SELECT _count(w_s1.a), _suml(w_s1.a) FROM w_s1 WHERE w_s1.a > 0",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        "ts",
        100,
        move |p| {
            let ptr = p.u8_ptr();
            // key is the ts column here; read first row only (single key per window)
            let key: i64 = unsafe { *(ptr as *const i64) };
            let cnt: i64 = unsafe { *(ptr.add(8) as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(16) as *const i64) };
            o2.lock().unwrap().push((p.window_start_ms(), key, cnt, sum));
        },
    )
    .unwrap();
    assert!(send_cols(sid, &[1000, 1])); // window [1000,1200)
    assert!(send_cols(sid, &[1500, 2])); // advances watermark to 1400
    assert!(send_cols(sid, &[1100, 3])); // late: ts=1100 < watermark 1400 -> dropped
    assert!(send_cols(sid, &[1300, 4])); // window [1200,1400): 1300 >= 1200, kept
    thread::sleep(Duration::from_millis(150));

    let out = out.lock().unwrap().clone();
    assert_eq!(out.len(), 3, "three windows, got {out:?}");
    let w1 = out.iter().find(|r| r.0 == 1000).unwrap();
    assert_eq!(w1.3, 1, "window [1000,1200) must contain only a=1 (late a=3 dropped)");
    let w2 = out.iter().find(|r| r.0 == 1200).unwrap();
    assert_eq!(w2.3, 4, "window [1200,1400) contains a=4");
    let w3 = out.iter().find(|r| r.0 == 1400).unwrap();
    assert_eq!(w3.3, 2, "window [1400,1600) contains a=2");
    stop();
}
