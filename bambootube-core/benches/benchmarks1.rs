mod bench_aux;
mod bench_log;

use criterion::{criterion_group, criterion_main, Bencher, Criterion};
use bambootube::{Column, U8Bytes};
use std::{ptr, thread, time::Duration};

const FILTER_SQL: &str = r#"
    SELECT demo.a, demo.b, demo.c, _sub(_add(demo.d, demo.d), demo.e)
    FROM demo
    WHERE (demo.a = 1 AND demo.b = 2) OR (demo.a = 3 AND demo.b = 4)
    LIMIT 10
    "#;

fn alternate_measurement() -> Criterion {
    Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5))
}

pub fn criterion_benchmark(c: &mut Criterion) {
    bench_log::setup_bambootube_home();
    bench_log::init_logger();
    bambootube::start();
    test_bambootube(c);
    bambootube::stop();
}

fn test<F>(c: &mut Criterion, test_case: &str, f: F)
where
    F: FnMut(&mut Bencher),
{
    call_test_case(c, test_case, f);
}

fn call_test_case<F>(c: &mut Criterion, test_case: &str, f: F)
where
    F: FnMut(&mut Bencher),
{
    c.bench_function(test_case, f);
}

criterion_group!(
    name = benches;
    config = alternate_measurement();
    targets = criterion_benchmark
);
criterion_main!(benches);

fn test_bambootube(c: &mut Criterion) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    if let Some((id1, _id2)) = define_records() {
        if let Some(mapper_id) = bambootube::def_mapper(FILTER_SQL, |_data, _size| {}) {
            log::info!("define mapper: {mapper_id}");
        } else {
            log::warn!("def_mapper failed");
            return;
        }
        let u8data = gen_u8_bytes(id1);
        test(c, "new_data_and_filter", |b| {
            b.iter(|| bambootube::new_data(&u8data))
        });
    }
}

fn define_records() -> Option<(u16, u16)> {
    let id1;
    let id2;
    if let Some(id) = bambootube::def_incoming(
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
    if let Some(id) = bambootube::def_stream(
        "stream",
        vec![
            Column::new_long("a"),
            Column::new_long("b"),
            Column::new_long("c"),
            Column::new_double("d"),
            Column::new_double("e"),
            Column::new_double("f"),
            Column::new_double("g"),
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

fn gen_u8_bytes(id: u16) -> U8Bytes {
    let mut u8array = [0_u8; 512];
    let slice = u8array.as_mut_slice();
    fill_u64(&mut slice[0..8], 1);
    fill_u64(&mut slice[8..16], 2);
    fill_u64(&mut slice[16..24], 3);
    fill_u64(&mut slice[24..32], 4);
    fill_u64(&mut slice[32..40], 5);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *mut u64;
    unsafe { *p_u64 = data };
}
