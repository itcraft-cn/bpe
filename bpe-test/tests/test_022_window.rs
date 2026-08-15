use bpe::{
    def_stream, def_window_aggregate, new_data, start, stop, Column, U8Bytes, Window,
};
use std::{
    env,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// Window tests exercise the timer thread, so they must run serially.
static ENGINE_LOCK: Mutex<()> = Mutex::new(());

fn lock_engine() -> std::sync::MutexGuard<'static, ()> {
    ENGINE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// (start_ms, end_ms, count, sum) collected from window callbacks.
type WinResult = (i64, i64, i64, i64);

/// Sends one record with columns ts(long)@0, a(long)@8, b(long)@16.
/// The id is the STREAM record id (the SQL FROM target of the window aggregate):
/// window aggregates listen directly on that record.
fn send_tab(id: u16, ts: i64, a: i64, b: i64) -> bool {
    let mut vec = vec![0_u8; 512];
    let ptr = vec.as_mut_ptr();
    unsafe {
        *(ptr as *mut i64) = ts;
        *(ptr.add(8) as *mut i64) = a;
        *(ptr.add(16) as *mut i64) = b;
    }
    new_data(&U8Bytes::new_from_vec(id, 512, vec))
}

/// Fixed (tumbling) window with event time: ts=1000,1100 -> [1000,1200);
/// ts=1300 -> [1200,1400); ts=1400 -> [1400,1600). All event times are in the
/// past, so every bucket is due on the first timer tick.
#[test]
fn test_tumbling_event_time() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let in_id = def_stream(
        "win_s1",
        vec![
            Column::new_long("ts"),
            Column::new_long("a"),
            Column::new_long("b"),
        ],
    )
    .unwrap();
    let results: Arc<Mutex<Vec<WinResult>>> = Arc::new(Mutex::new(vec![]));
    let r2 = results.clone();
    def_window_aggregate(
        "SELECT _count(win_s1.a), _suml(win_s1.a) FROM win_s1 WHERE win_s1.a > 0",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let cnt: i64 = unsafe { *(ptr as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r2.lock()
                .unwrap()
                .push((p.window_start_ms(), p.window_end_ms(), cnt, sum));
        },
    )
    .unwrap();
    assert!(send_tab(in_id, 1000, 5, 0));
    assert!(send_tab(in_id, 1100, 7, 0));
    assert!(send_tab(in_id, 1300, 11, 0));
    assert!(send_tab(in_id, 1400, 13, 0));
    thread::sleep(Duration::from_millis(150)); // let the timer deliver

    let mut out = results.lock().unwrap().clone();
    out.sort();
    assert_eq!(out.len(), 3, "three windows expected, got {out:?}");
    assert_eq!(out[0], (1000, 1200, 2, 12), "window1 [1000,1200) a=5,7");
    assert_eq!(out[1], (1200, 1400, 1, 11), "window2 [1200,1400) a=11");
    assert_eq!(out[2], (1400, 1600, 1, 13), "window3 [1400,1600) a=13");
    stop();
}

/// Fixed window with processing time (no ts field): records are bucketed by
/// arrival time; the timer thread fires the window after its period elapses.
#[test]
fn test_tumbling_processing_time() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let in_id = def_stream("win_s2", vec![Column::new_long("ts"), Column::new_long("a")]).unwrap();
    let results: Arc<Mutex<Vec<WinResult>>> = Arc::new(Mutex::new(vec![]));
    let r2 = results.clone();
    def_window_aggregate(
        "SELECT _count(win_s2.a), _suml(win_s2.a) FROM win_s2 WHERE win_s2.a > 0",
        Window::Tumbling { period_ms: 300 },
        None,
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let cnt: i64 = unsafe { *(ptr as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r2.lock()
                .unwrap()
                .push((p.window_start_ms(), p.window_end_ms(), cnt, sum));
        },
    )
    .unwrap();
    assert!(send_tab(in_id, 0, 1, 0)); // window A
    thread::sleep(Duration::from_millis(400)); // cross into window B
    assert!(send_tab(in_id, 0, 2, 0)); // window B
    thread::sleep(Duration::from_millis(450)); // let window B expire

    let out = results.lock().unwrap().clone();
    assert_eq!(out.len(), 2, "two processing-time windows, got {out:?}");
    let mut sums: Vec<i64> = out.iter().map(|r| r.2).collect();
    sums.sort();
    assert_eq!(sums, vec![1, 1], "each window holds one record");
    let mut total: i64 = out.iter().map(|r| r.3).collect::<Vec<_>>().iter().sum();
    let _ = &mut total;
    assert_eq!(
        out.iter().map(|r| r.3).sum::<i64>(),
        3,
        "sum of all window sums = 1 + 2"
    );
    stop();
}

/// Sliding window: length=300ms, slide=150ms.
/// ts=1000 -> [750,1050)+[900,1200); ts=1100 -> [900,1200)+[1050,1350);
/// ts=1300 -> [1050,1350)+[1200,1500).
#[test]
fn test_sliding_window() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let in_id = def_stream("win_s3", vec![Column::new_long("ts"), Column::new_long("a")]).unwrap();
    let results: Arc<Mutex<Vec<WinResult>>> = Arc::new(Mutex::new(vec![]));
    let r2 = results.clone();
    def_window_aggregate(
        "SELECT _count(win_s3.a), _suml(win_s3.a) FROM win_s3 WHERE win_s3.a > 0",
        Window::Sliding {
            length_ms: 300,
            slide_ms: 150,
        },
        Some("ts"),
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let cnt: i64 = unsafe { *(ptr as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r2.lock()
                .unwrap()
                .push((p.window_start_ms(), p.window_end_ms(), cnt, sum));
        },
    )
    .unwrap();
    assert!(send_tab(in_id, 1000, 1, 0));
    assert!(send_tab(in_id, 1100, 2, 0));
    assert!(send_tab(in_id, 1300, 3, 0));
    thread::sleep(Duration::from_millis(150));

    let mut out = results.lock().unwrap().clone();
    out.sort();
    assert_eq!(out.len(), 4, "four sliding windows, got {out:?}");
    assert_eq!(out[0], (750, 1050, 1, 1), "window [750,1050) contains a=1");
    assert_eq!(out[1], (900, 1200, 2, 3), "window [900,1200) contains a=1,2");
    assert_eq!(out[2], (1050, 1350, 2, 5), "window [1050,1350) contains a=2,3");
    assert_eq!(out[3], (1200, 1500, 1, 3), "window [1200,1500) contains a=3");
    stop();
}

/// WHERE filtering applies before bucketing; an empty window still fires with
/// size()==0 and initial aggregate values.
#[test]
fn test_window_filter_and_empty() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let in_id = def_stream("win_s4", vec![Column::new_long("ts"), Column::new_long("a")]).unwrap();
    let results: Arc<Mutex<Vec<(i64, i64, i64, i64, usize)>>> = Arc::new(Mutex::new(vec![]));
    let r2 = results.clone();
    def_window_aggregate(
        "SELECT _count(win_s4.a), _suml(win_s4.a) FROM win_s4 WHERE win_s4.a > 5",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let cnt: i64 = unsafe { *(ptr as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r2.lock().unwrap().push((
                p.window_start_ms(),
                p.window_end_ms(),
                cnt,
                sum,
                p.size(),
            ));
        },
    )
    .unwrap();
    // window [1000,1200): a=3 filtered out, a=7 kept
    assert!(send_tab(in_id, 1000, 3, 0));
    assert!(send_tab(in_id, 1100, 7, 0));
    // window [1200,1400): nothing passes the filter -> empty window
    assert!(send_tab(in_id, 1300, 1, 0));
    assert!(send_tab(in_id, 1400, 2, 0));
    thread::sleep(Duration::from_millis(150));

    let mut out = results.lock().unwrap().clone();
    out.sort();
    // [1000,1200) filtered (1 record), [1200,1400) empty, [1400,1600) empty
    assert_eq!(out.len(), 3, "three windows (one with data, two empty), got {out:?}");
    // [1000,1200): count=1, sum=7, size=1
    let w1 = out.iter().find(|r| r.0 == 1000).unwrap();
    assert_eq!(*w1, (1000, 1200, 1, 7, 1), "filtered window");
    // [1200,1400): empty window fires with size=0 and initial values
    let w2 = out.iter().find(|r| r.0 == 1200).unwrap();
    assert_eq!(w2.2, 0, "empty window count must be 0");
    assert_eq!(w2.3, 0, "empty window sum must be 0");
    assert_eq!(w2.4, 0, "empty window size must be 0");
    // [1400,1600): also empty (the rejected ts=1400 record's window)
    let w3 = out.iter().find(|r| r.0 == 1400).unwrap();
    assert_eq!(w3.4, 0, "second empty window must also fire");
    stop();
}

/// Multiple window aggregates on the same record are independent.
#[test]
fn test_multiple_window_aggregates() {
    let _g = lock_engine();
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let in_id = def_stream(
        "win_s5",
        vec![Column::new_long("ts"), Column::new_long("a"), Column::new_long("b")],
    )
    .unwrap();
    let res1: Arc<Mutex<Vec<WinResult>>> = Arc::new(Mutex::new(vec![]));
    let res2: Arc<Mutex<Vec<WinResult>>> = Arc::new(Mutex::new(vec![]));
    let r1 = res1.clone();
    let r2 = res2.clone();
    def_window_aggregate(
        "SELECT _count(win_s5.a), _suml(win_s5.a) FROM win_s5 WHERE win_s5.a > 0",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let cnt: i64 = unsafe { *(ptr as *const i64) };
            let sum: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r1.lock().unwrap().push((p.window_start_ms(), p.window_end_ms(), cnt, sum));
        },
    )
    .unwrap();
    def_window_aggregate(
        "SELECT _maxl(win_s5.b), _minl(win_s5.b) FROM win_s5 WHERE win_s5.b > 0",
        Window::Tumbling { period_ms: 200 },
        Some("ts"),
        0,
        move |p| {
            let ptr = p.u8_ptr();
            let mx: i64 = unsafe { *(ptr as *const i64) };
            let mn: i64 = unsafe { *(ptr.add(8) as *const i64) };
            r2.lock().unwrap().push((p.window_start_ms(), p.window_end_ms(), mx, mn));
        },
    )
    .unwrap();
    // [1000,1200): a=5,7 / b=10,20
    assert!(send_tab(in_id, 1000, 5, 10));
    assert!(send_tab(in_id, 1100, 7, 20));
    thread::sleep(Duration::from_millis(150));

    let o1 = res1.lock().unwrap().clone();
    let o2 = res2.lock().unwrap().clone();
    assert_eq!(o1.len(), 1, "agg1 must fire once");
    assert_eq!(o1[0], (1000, 1200, 2, 12), "agg1 count=2 sum=12");
    assert_eq!(o2.len(), 1, "agg2 must fire once");
    assert_eq!(o2[0], (1000, 1200, 20, 10), "agg2 max=20 min=10");
    stop();
}
