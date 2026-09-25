// Verify record_size effect: 8 columns = 64B payload vs 512B default.
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{env, time::Instant};

fn main() {
    // rsize 为可选位置参数：缺省时使用 512B（默认记录大小），
    // 便于 `cargo run --example=perf_rsize` 直接运行。
    let rsize: usize = env::args()
        .nth(1)
        .map(|s| s.parse().expect("rsize 必须是正整数"))
        .unwrap_or(512);
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming("pr_demo", vec![
        Column::new_long("ts"), Column::new_long("user_id"), Column::new_long("amount"),
        Column::new_long("risk"), Column::new_long("a"), Column::new_long("b"),
        Column::new_long("c"), Column::new_long("d"),
    ]).unwrap();
    def_mapper(
        "SELECT pr_demo.amount FROM pr_demo WHERE pr_demo.amount > 100 AND pr_demo.risk = 1 AND pr_demo.a > 0 AND pr_demo.b > 0 AND pr_demo.c > 0 AND pr_demo.d > 0 LIMIT 10",
        |_p| {},
    ).unwrap();
    let mut u8data = U8Bytes::new_from_slice(rid, rsize, &vec![0_u8; rsize]);
    let bytes = u8data.bytes_mut();
    let ptr = bytes.as_mut_ptr();
    for i in 0..50_000i64 {
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let start = Instant::now();
    for i in 0..1_000_000i64 {
        unsafe { *(ptr.add(16) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let el = start.elapsed();
    eprintln!("bpe record_size={rsize}: {:.1} ns/op ({:.0} events/s)",
        el.as_nanos() as f64 / 1e6, 1e6 / el.as_secs_f64());
    stop();
}
