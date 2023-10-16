use log::*;
use shower::{def_action_lua, new_data, start, stop, U8Bytes};
use std::{env, thread};

const LOOP_SIZE: usize = 1000000;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    def_action_lua(
        r#"
        function(data_ptr)
            _select(_id(data_ptr),
                _mix_array_to_u128(1, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
                _mix_array_to_u128(0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
                3)
        end
        "#,
    );
    gen_new_data();
    stop();
}

fn gen_new_data() {
    let core_ids = core_affinity::get_core_ids().unwrap();
    core_affinity::set_for_current(core_ids[core_ids.len() - 1]);
    info!("thread:{} started", thread::current().name().unwrap());
    let u8data = U8Bytes::new_from_vec(16, 288, vec![0u8; 288]);
    for _ in 1..=LOOP_SIZE {
        let ret = new_data(&u8data);
        if ret {
            debug!("send success");
        } else {
            warn!("send failed");
        }
    }
}
