// Aggregate cost isolation: window-less bind-aggregate path (interpreter).
use bpe::{def_aggregate, def_incoming, def_mapper_bind_aggregate, def_stream, new_data, start, stop, Column, U8Bytes};
use std::{env, time::Instant};

fn main() {
    env::set_var("BPE_HOME", env::current_dir().unwrap());
    start();
    let rid = def_incoming("pa_demo", vec![
        Column::new_long("ts"), Column::new_long("amount"), Column::new_long("risk"),
    ]).unwrap();
    def_stream("pa_stream", vec![
        Column::new_long("a"), Column::new_long("b"), Column::new_long("c"),
        Column::new_long("d"), Column::new_long("e"),
    ]).unwrap();
    let agg_id = def_aggregate(
        "select _suml(pa_stream.a), _count(pa_stream.a), _avg(pa_stream.a), _maxl(pa_stream.a), _minl(pa_stream.a) from pa_stream",
        |_| {},
    ).unwrap();
    def_mapper_bind_aggregate(
        "SELECT pa_demo.amount FROM pa_demo WHERE pa_demo.amount > 100 LIMIT 10",
        agg_id,
    ).unwrap();
    let mut u8data = U8Bytes::new_from_slice(rid, 512, &vec![0_u8; 512]);
    let bytes = u8data.bytes_mut();
    let ptr = bytes.as_mut_ptr();
    for i in 0..50_000i64 {
        unsafe { *(ptr.add(8) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let start = Instant::now();
    for i in 0..1_000_000i64 {
        unsafe { *(ptr.add(8) as *mut i64) = 1000 + (i % 100) };
        new_data(&u8data);
    }
    let el = start.elapsed();
    eprintln!("perf_agg: {:.1} ns/op ({} ops/s)", el.as_nanos() as f64 / 1_000_000.0, 1e6 / el.as_secs_f64());
    stop();
}
