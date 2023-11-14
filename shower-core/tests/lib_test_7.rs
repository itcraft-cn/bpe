use shower::{
    def_aggregate, def_incoming, def_mapper_with_aggregate, new_data, start, stop, Column, U8Bytes,
};
use std::{env, ptr, thread};

const LOOP_SIZE: usize = 10;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    let id = def_incoming(vec![
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_long(),
        Column::new_string(240),
    ]);
    log::info!("defined record: {:?}", id);
    def_aggregate("select 1 from 1", |data| {
        log::info!("data len: [{}]", data.len());
    });
    def_mapper_with_aggregate("select _1.__1 from _1 limit 1", 1);
    gen_new_data(id);
    stop();
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
    let mut u8array = [0u8; 512];
    let slice = u8array.as_mut_slice();
    fill_u64(&mut slice[0..8], 1);
    fill_u64(&mut slice[8..16], 2);
    fill_u64(&mut slice[16..24], 3);
    fill_u64(&mut slice[24..32], 4);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *mut u64;
    unsafe { *p_u64 = data };
}
