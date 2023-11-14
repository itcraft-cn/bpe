use shower::{def_mapper, new_data, start, stop, U8Bytes};
use std::{env, thread};

const LOOP_SIZE: usize = 10;

const SQL: &str = r#"
    SELECT _1.__1, _1.__2, _1.__3, _sub(_add(_1.__4, _1.__4), _1.__5)
    FROM _1
    WHERE (_1.__1 = '1' AND _1.__2 = '2') OR (_1.__1 = '3' AND _1.__2 = '4')
    LIMIT 10
    "#;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_mapper(SQL, |_vec| {});
    gen_new_data();
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    log::info!("thread:{} started", thread::current().name().unwrap());
    let u8data = U8Bytes::new_from_vec(1, 288, vec![0u8; 288]);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            log::debug!("send success");
        } else {
            log::warn!("send failed");
        }
    }
}
