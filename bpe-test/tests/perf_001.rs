// Performance profiling entry: simulates the risk-control hot path.
// Run with: cargo flamegraph --dev --no-inline --test perf_001 -- perf_filter --nocapture
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{
    env,
    time::Instant,
};

const LOOP_SIZE: usize = 2_000_000;

/// 8-column record; filter with 4 conditions + computed fields.
#[test]
fn perf_filter() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming(
        "perf_demo",
        vec![
            Column::new_long("ts"),
            Column::new_long("user_id"),
            Column::new_long("amount"),
            Column::new_long("risk"),
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_long("d"),
        ],
    )
    .unwrap();
    def_mapper(
        "SELECT perf_demo.user_id, _to_double(perf_demo.amount), _add(perf_demo.a, perf_demo.b), _sub(perf_demo.c, perf_demo.d), _abs(perf_demo.amount) \
         FROM perf_demo \
         WHERE perf_demo.amount > 100 AND perf_demo.risk = 1 AND perf_demo.a > 0 AND perf_demo.b > 0 AND perf_demo.c > 0 AND perf_demo.d > 0 \
         LIMIT 10",
        |_p| {},
    )
    .unwrap();

    let mut payload = vec![0_u8; 512];
    // reuse one U8Bytes instance: mutate its payload between new_data calls
    let mut u8data = U8Bytes::new_from_slice(rid, 512, &payload);
    let bytes = u8data.bytes_mut();
    let ptr = bytes.as_mut_ptr();
    // warmup (JIT compilation happens on first call)
    for i in 0..50_000 {
        unsafe {
            *(ptr as *mut i64) = 1_700_000_000_000_i64 + i as i64;
            *(ptr.add(8) as *mut i64) = (i % 1000) as i64; // user_id
            *(ptr.add(16) as *mut i64) = 1000 + (i % 100) as i64; // amount
            *(ptr.add(24) as *mut i64) = 1; // risk
            *(ptr.add(32) as *mut i64) = 1 + (i % 50) as i64; // a
            *(ptr.add(40) as *mut i64) = 1 + (i % 50) as i64; // b
            *(ptr.add(48) as *mut i64) = 1 + (i % 50) as i64; // c
            *(ptr.add(56) as *mut i64) = 1 + (i % 50) as i64; // d
        }
        new_data(&u8data);
    }

    let start = Instant::now();
    for i in 0..LOOP_SIZE {
        unsafe {
            *(ptr as *mut i64) = 1_700_000_000_000_i64 + i as i64;
            *(ptr.add(8) as *mut i64) = (i % 1000) as i64;
            *(ptr.add(16) as *mut i64) = 1000 + (i % 100) as i64;
            *(ptr.add(24) as *mut i64) = 1;
            *(ptr.add(32) as *mut i64) = 1 + (i % 50) as i64;
            *(ptr.add(40) as *mut i64) = 1 + (i % 50) as i64;
            *(ptr.add(48) as *mut i64) = 1 + (i % 50) as i64;
            *(ptr.add(56) as *mut i64) = 1 + (i % 50) as i64;
        }
        new_data(&u8data);
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() as f64 / LOOP_SIZE as f64;
    eprintln!(
        "perf_filter: {} ops, {:.2?} total, {:.1} ns/op, {:.0} ops/s",
        LOOP_SIZE,
        elapsed,
        per_op,
        LOOP_SIZE as f64 / elapsed.as_secs_f64()
    );
    stop();
}
