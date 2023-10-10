use std::sync::Once;

use crate::{aux::fill_u64, cfg::get_config, consts::KEY_STORED_TICK_SIZE, Tick};
use hashbrown::HashMap;

const TICK_SIZE: usize = 64;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<HashMap<u16, WrappedArray>> = None;

static mut TICK_MAX_SIZE: usize = 0;
static mut VEC_SIZE: usize = 0;
static mut VEC_MASK: usize = 0;

pub(crate) fn insert(tick: Tick) {
    MAP_INIT.call_once(|| unsafe {
        MAP.get_or_insert(HashMap::new());
        TICK_MAX_SIZE = get_config().fetch_cfg_usize(KEY_STORED_TICK_SIZE);
        VEC_SIZE = TICK_SIZE * TICK_MAX_SIZE;
        VEC_MASK = VEC_SIZE - 1;
    });
    unsafe {
        insert_into_slice(tick);
    }
}

unsafe fn insert_into_slice(tick: Tick) {
    let map = MAP.as_mut().unwrap();
    let array = map
        .entry(tick.quote_id)
        .or_insert_with(|| WrappedArray::new(vec![0u8; VEC_SIZE]));
    let slice = array.data.as_mut_slice();
    fill_u64(&mut slice[array.walker..array.walker + 8], tick.bid);
    fill_u64(&mut slice[array.walker + 8..array.walker + 16], tick.ask);
    fill_u64(&mut slice[array.walker + 16..array.walker + 24], tick.last);
    fill_u64(
        &mut slice[array.walker + 24..array.walker + 32],
        tick.volume,
    );
    fill_u64(
        &mut slice[array.walker + 32..array.walker + 40],
        tick.timestamp,
    );
    array.walker = (array.walker + TICK_SIZE) & VEC_MASK;
}

struct WrappedArray {
    data: Vec<u8>,
    walker: usize,
}

impl WrappedArray {
    fn new(data: Vec<u8>) -> Self {
        WrappedArray {
            data: data,
            walker: 0,
        }
    }
}
