use std::sync::Once;

use crate::{
    aux::fill_u64,
    cfg::get_config,
    consts::{KEY_STORED_TICK_SIZE, MAX_DEEP_TICK_DEPTH},
    DeepTick, Tick,
};
use hashbrown::HashMap;

const TICK_SIZE: usize = 64;
const _DEEP_TICK_SIZE: usize = 512;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<HashMap<u16, WrappedArray>> = None;

static mut STORED_TICK_SIZE: usize = 0;

static mut TICK_VEC_SIZE: usize = 0;
static mut TICK_VEC_MASK: usize = 0;

static mut _DEEP_TICK_VEC_SIZE: usize = 0;
static mut _DEEP_TICK_VEC_MASK: usize = 0;

pub(crate) fn insert(tick: Tick) {
    MAP_INIT.call_once(|| unsafe { initial() });
    unsafe {
        insert_into_slice(tick);
    }
}

unsafe fn insert_into_slice(tick: Tick) {
    let map = MAP.as_mut().unwrap();
    let array = map
        .entry(tick.quote_id)
        .or_insert_with(|| WrappedArray::new(vec![0u8; TICK_VEC_SIZE]));
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
    array.walker = (array.walker + TICK_SIZE) & TICK_VEC_MASK;
}

pub(crate) fn _insert_deep(tick: DeepTick) {
    MAP_INIT.call_once(|| unsafe { initial() });
    unsafe {
        _insert_deep_into_slice(tick);
    }
}

unsafe fn _insert_deep_into_slice(tick: DeepTick) {
    let map = MAP.as_mut().unwrap();
    let array = map
        .entry(tick.quote_id)
        .or_insert_with(|| WrappedArray::new(vec![0u8; _DEEP_TICK_VEC_SIZE]));
    let slice = array.data.as_mut_slice();
    _fill_u64_array(
        &mut slice[array.walker..array.walker + 240],
        tick.bid.as_slice(),
    );
    _fill_u64_array(
        &mut slice[array.walker + 240..array.walker + 480],
        tick.ask.as_slice(),
    );
    fill_u64(
        &mut slice[array.walker + 480..array.walker + 488],
        tick.last,
    );
    fill_u64(
        &mut slice[array.walker + 488..array.walker + 496],
        tick.volume,
    );
    fill_u64(
        &mut slice[array.walker + 496..array.walker + 504],
        tick.timestamp,
    );
    array.walker = (array.walker + _DEEP_TICK_SIZE) & _DEEP_TICK_VEC_MASK;
}

fn _fill_u64_array(slice: &mut [u8], array: &[u64]) {
    for i in 0..MAX_DEEP_TICK_DEPTH {
        fill_u64(&mut slice[i * 8..(i + 1) * 8], array[i]);
    }
}

unsafe fn initial() {
    MAP.get_or_insert(HashMap::new());
    STORED_TICK_SIZE = get_config().fetch_cfg_usize(KEY_STORED_TICK_SIZE);
    TICK_VEC_SIZE = TICK_SIZE * STORED_TICK_SIZE;
    TICK_VEC_MASK = TICK_VEC_SIZE - 1;
    _DEEP_TICK_VEC_SIZE = _DEEP_TICK_SIZE * STORED_TICK_SIZE;
    _DEEP_TICK_VEC_MASK = _DEEP_TICK_VEC_SIZE - 1;
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
