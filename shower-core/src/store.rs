use std::sync::Once;

use crate::Tick;
use hashbrown::HashMap;

const TICK_SIZE: usize = 64;
const TICK_MAX_SIZE: usize = 65536;
const VEC_SIZE: usize = TICK_SIZE * TICK_MAX_SIZE;
const VEC_MASK: usize = VEC_SIZE - 1;

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
            .or_insert_with(|| WrappedArray::new(vec![0u8; VEC_SIZE]));
        let slice = array.data.as_mut_slice();
        write(&mut slice[array.walker..array.walker + 8], tick.bid);
        write(&mut slice[array.walker + 8..array.walker + 16], tick.ask);
        write(&mut slice[array.walker + 16..array.walker + 24], tick.last);
        write(
            &mut slice[array.walker + 24..array.walker + 32],
            tick.volume,
        );
        write(
            &mut slice[array.walker + 32..array.walker + 40],
            tick.timestamp,
        );
        array.walker = (array.walker + TICK_SIZE) & VEC_MASK;
    }
}

fn write(slice: &mut [u8], data: u64) {
    slice[0] = (data >> 0) as u8;
    slice[1] = (data >> 1) as u8;
    slice[2] = (data >> 2) as u8;
    slice[3] = (data >> 3) as u8;
    slice[4] = (data >> 4) as u8;
    slice[5] = (data >> 5) as u8;
    slice[6] = (data >> 6) as u8;
    slice[7] = (data >> 7) as u8;
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
