// Isolate the always-false filter scan cost (single mapper).
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{env, time::Instant};

fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming(
        "pc_demo",
        vec![
            Column::new_long("ts"), Column::new_long("user_id"), Column::new_long("amount"),
            Column::new_long("risk"), Column::new_long("a"), Column::new_long("b"),
            Column::new_long("c"), Column::new_long("d"),
        ],
    )
    .unwrap();
    def_mapper(
        "SELECT pc_demo.a, pc_demo.b FROM pc_demo WHERE pc_demo.amount > 900000000000000000 LIMIT 10",
        |_p| {},
    )
    .unwrap();
    let mut u8data = U8Bytes::new_from_slice(rid, 512, &vec![0_u8; 512]);
    let bytes = u8data.bytes_mut();
    let ptr = bytes.as_mut_ptr();
    for i in 0..50_000i64 {
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let start = Instant::now();
    for i in 0..2_000_000i64 {
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let el = start.elapsed();
    eprintln!("perf_c: {:.1} ns/op", el.as_nanos() as f64 / 2_000_000.0);
    stop();
}
