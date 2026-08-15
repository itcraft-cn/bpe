// Differential profiling: isolates insert / JIT-filter / field-fetch / callback costs.
// Run: cargo run --release --bin perf_diff
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{
    env,
    time::Instant,
};

const LOOP: usize = 1_000_000;

fn bench(name: &str, mut f: impl FnMut()) -> f64 {
    // warmup
    for _ in 0..50_000 {
        f();
    }
    let start = Instant::now();
    for _ in 0..LOOP {
        f();
    }
    let per = start.elapsed().as_nanos() as f64 / LOOP as f64;
    eprintln!("{name}: {per:.1} ns/op");
    per
}

fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming(
        "pd_demo",
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

    let mut u8data = U8Bytes::new_from_slice(rid, 512, &vec![0_u8; 512]);
    let bytes = u8data.bytes_mut();
    let ptr = bytes.as_mut_ptr();
    let mut i: i64 = 0;

    // A: no mapper -> pure insert
    let a = bench("A insert-only", || {
        i += 1;
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    });

    // C: mapper with always-false WHERE -> insert + filter.call (no fetch)
    def_mapper(
        "SELECT pd_demo.a, pd_demo.b FROM pd_demo WHERE pd_demo.amount > 900000000000000000 LIMIT 10",
        |_p| {},
    )
    .unwrap();
    let c = bench("C insert+filter(always-false)", || {
        i += 1;
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    });

    // B: mapper with mostly-true WHERE -> insert + filter + fetch + callback
    def_mapper(
        "SELECT pd_demo.user_id, _to_double(pd_demo.amount), _add(pd_demo.a, pd_demo.b), _sub(pd_demo.c, pd_demo.d), _abs(pd_demo.amount) \
         FROM pd_demo WHERE pd_demo.amount > 100 LIMIT 10",
        |_p| {},
    )
    .unwrap();
    let b = bench("B insert+filter+fetch+callback", || {
        i += 1;
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    });

    eprintln!("--- breakdown (ns/op) ---");
    eprintln!("insert:           {a:.1}");
    eprintln!("filter.call:      {:.1}   (C - A)", c - a);
    eprintln!("fetch+callback:   {:.1}   (B - C)", b - c);
    eprintln!("total (B):        {b:.1}");
    stop();
}
