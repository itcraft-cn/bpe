use shower::{def_incoming, new_data, start, stop, Column, U8Bytes};
use std::{env, thread};

const LOOP_SIZE: usize = 100;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    gen_new_data();
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    if let Some(id) = def_incoming(
        "demo",
        vec![
            Column::new_long("a"),
            Column::new_double("b"),
            Column::new_long("c"),
            Column::new_double("d"),
            Column::new_string("e", 240),
        ],
    ) {
        let u8data = U8Bytes::new_from_vec(id, 288, vec![0_u8; 288]);
        for _ in 1..=LOOP_SIZE {
            let ret = new_data(&u8data);
            if ret {
                log::debug!("send success");
            } else {
                log::warn!("send failed");
            }
        }
    }
}
