// Aligned with Esper: filter + 5 computed fields, output 1 row per event (LIMIT 1).
use bpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::{env, time::Instant};

fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming("pa_demo", vec![
        Column::new_long("ts"), Column::new_long("user_id"), Column::new_long("amount"),
        Column::new_long("risk"), Column::new_long("a"), Column::new_long("b"),
        Column::new_long("c"), Column::new_long("d"),
    ]).unwrap();
    def_mapper(
        "SELECT pa_demo.user_id, _to_double(pa_demo.amount), _add(pa_demo.a, pa_demo.b), _sub(pa_demo.c, pa_demo.d), _mul(pa_demo.a, 2) FROM pa_demo WHERE pa_demo.amount > 100 LIMIT 1",
        |_p| {},
    ).unwrap();
    let mut u8data = U8Bytes::new_from_slice(rid, 512, &vec![0_u8; 512]);
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
    eprintln!("bpe filter+5fields(limit1): {:.1} ns/op ({:.0} events/s)",
        el.as_nanos() as f64 / 1e6, 1e6 / el.as_secs_f64());
    stop();
}
