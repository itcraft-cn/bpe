mod test_aux;
mod test_log;

use bbpe::{def_incoming, def_mapper, new_data, start, stop, Column, U8Bytes};
use std::thread;
use test_aux::{fetch_ptr, fill_f64};
use test_log::{init_logger, setup_bbpe_home};

const LOOP_SIZE: usize = 20;

const FILTER_SQL: &str = r#"
    select demo.a, demo.b from demo limit 10
    "#;

#[test]
fn test_new_proc() {
    setup_bbpe_home();
    init_logger();
    start();
    if let Some(id) = define_records() {
        if let Some(id) = def_mapper(FILTER_SQL, |data, size| {
            log::info!("fetched data: {size}");
            for idx in 0..size {
                unsafe {
                    log::info!("b:{}", fetch_ptr::<f64>(data.add(idx * 512 + 8)));
                }
            }
        }) {
            log::warn!("def_mapper[{id}] success");
        } else {
            log::warn!("def_mapper failed");
            return;
        }
        gen_new_data(id);
    }
    stop();
}

fn define_records() -> Option<u16> {
    if let Some(id) = def_incoming(
        "demo",
        vec![Column::new_double("a"), Column::new_double("b")],
    ) {
        log::info!("defined incoming: {id}");
        Some(id)
    } else {
        log::warn!("failed to define incoming record");
        None
    }
}

fn gen_new_data(id: u16) {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    let u8data = gen_u8_bytes(id);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            log::debug!("send success");
        } else {
            log::warn!("send failed");
        }
    }
}

fn gen_u8_bytes(id: u16) -> U8Bytes {
    let mut u8array = [0_u8; 512];
    let slice = u8array.as_mut_slice();
    fill_f64(&mut slice[0..8], 1_f64);
    fill_f64(&mut slice[8..16], 2_f64);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}
