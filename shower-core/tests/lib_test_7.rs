mod test_aux;

use shower::{
    def_aggregate, def_incoming, def_mapper_bind_aggregate, def_stream, new_data, start, stop,
    Column, U8Bytes,
};
use std::{env, thread};
use test_aux::fill_i64;

const LOOP_SIZE: usize = 3;

const FILTER_SQL: &str = r#"
    select demo.a from demo limit 10
    "#;
const AGGREGATE_SQL: &str = r#"
    select _maxl(stream.a), _minl(stream.a), _suml(stream.a),
           _maxd(stream.a), _mind(stream.a), _sumd(stream.a),
           _avgd(stream.a), _countl(stream.a)
    from stream
    "#;

#[test]
fn test_new_proc() {
    env::set_var("SHOWER_HOME", "/home/helly/code/rust/shower");
    start();
    if let Some((id1, _id2)) = define_tables() {
        if let Some(aggregate_id) = def_aggregate(AGGREGATE_SQL, |data| {
            let len = data.len();
            log::info!("data len: [{}]", data.len());
            if len == 1 {
                log::info!("data: {:?}", &data[0].as_slice()[0..64]);
            }
        }){
            let opt = def_mapper_bind_aggregate(FILTER_SQL, aggregate_id);
            if opt.is_none() {
                log::warn!("def_mapper_bind_aggregate failed");
                return;
            }
        } else {
            log::warn!("def_aggregate failed");
            return;
        }
        gen_new_data(id1);
    }
    stop();
}

fn define_tables() -> Option<(u16, u16)> {
    let id1;
    let id2;
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
        id1 = id;
        log::info!("defined incoming: {}", id1);
    } else {
        log::warn!("failed to define incoming table");
        return None;
    }
    if let Some(id) = def_stream("stream", vec![Column::new_long("a")]) {
        id2 = id;
        log::info!("defined stream: {}", id2);
    } else {
        log::warn!("failed to define stream table");
        return None;
    }
    Some((id1, id2))
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
    fill_i64(&mut slice[0..8], 1);
    fill_i64(&mut slice[8..16], 2);
    fill_i64(&mut slice[16..24], 3);
    fill_i64(&mut slice[24..32], 4);
    U8Bytes::new_from_vec(id, 512, Vec::from(u8array))
}
