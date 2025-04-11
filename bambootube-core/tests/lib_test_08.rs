mod test_aux;
mod test_log;

use bambootube::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::thread;
use test_aux::{fetch_ptr, fill_f64, fill_i64};
use test_log::{init_logger, setup_bambootube_home};

const LOOP_SIZE: usize = 10;

const SQL: &str = r#"
    SELECT demo.a, demo.b, demo.c, demo.d, demo.e
    FROM demo
    LIMIT 10
    "#;

#[test]
fn test_new_proc() {
    setup_bambootube_home();
    init_logger();
    start();
    gen_new_data();
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    let columns = vec![
        Column::new_long("a"),
        Column::new_long("b"),
        Column::new_double("c"),
        Column::new_double("d"),
        Column::new_double("e"),
    ];
    let id = def_incoming("demo", columns).unwrap();
    def_mapper(SQL, |data, size| {
        log::info!("fetched data: {}", size);
        for idx in 0..size {
            unsafe {
                let v1 = fetch_ptr::<i64>(data.add(idx * 512));
                let v2 = fetch_ptr::<i64>(data.add(idx * 512 + 8));
                let v3 = fetch_ptr::<f64>(data.add(idx * 512 + 16));
                let v4 = fetch_ptr::<f64>(data.add(idx * 512 + 24));
                let v5 = fetch_ptr::<f64>(data.add(idx * 512 + 32));
                log::info!("{}/{}/{}/{}/{}", v1, v2, v3, v4, v5);
            }
        }
    });
    let mut vec = vec![0_u8; 512];
    let slice = vec.as_mut_slice();
    fill_i64(&mut slice[0..8], 1);
    fill_i64(&mut slice[8..16], 2);
    fill_f64(&mut slice[16..24], 1_f64);
    fill_f64(&mut slice[24..32], 2_f64);
    fill_f64(&mut slice[32..40], 3_f64);
    let u8data = U8Bytes::new_from_vec(id, 288, vec);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            log::debug!("send success");
        } else {
            log::warn!("send failed");
        }
    }
}
