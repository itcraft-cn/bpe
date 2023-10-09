use std::sync::Once;

use crate::Tick;
use hashbrown::HashMap;

const TICK_SIZE: usize = 40;
const TICK_MAX_SIZE: usize = 1000000;
const VEC_SIZE: usize = TICK_SIZE * TICK_MAX_SIZE;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<HashMap<u16, WrappedArray>> = None;

pub(crate) fn insert(tick: Tick) {
    MAP_INIT.call_once(|| unsafe {
        MAP.get_or_insert(HashMap::new());
    });
    unsafe {
        let map = MAP.as_mut().unwrap();
        let array = map
            .entry(tick.quote_id)
            .or_insert(WrappedArray::new(vec![0u8; VEC_SIZE]));
        let slice = array.data.as_mut_slice();
        slice[array.walker..array.walker + 8].copy_from_slice(&tick.bid.to_le_bytes());
        slice[array.walker + 8..array.walker + 16].copy_from_slice(&tick.ask.to_le_bytes());
        slice[array.walker + 16..array.walker + 24].copy_from_slice(&tick.last.to_le_bytes());
        slice[array.walker + 24..array.walker + 32].copy_from_slice(&tick.volume.to_le_bytes());
        slice[array.walker + 32..array.walker + 40].copy_from_slice(&tick.timestamp.to_le_bytes());
        array.walker += TICK_SIZE;
        if array.walker >= VEC_SIZE {
            array.walker -= VEC_SIZE;
        }
    }
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
