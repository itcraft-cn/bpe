use crate::{aux::fill_u64, data::U8Tick};
use hashbrown::HashMap;
use std::sync::Once;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<HashMap<u16, WrappedArray>> = None;

pub(crate) fn insert(tick: U8Tick) {
    MAP_INIT.call_once(|| unsafe { initial() });
    unsafe {
        insert_into_slice(tick);
    }
}

unsafe fn initial() {
    MAP.get_or_insert(HashMap::new());
}

unsafe fn insert_into_slice(tick: U8Tick) {
    let quote_id = tick.quote_id();
    let element_size = tick.element_size();
    let step = tick.tick_size();
    let size = tick.u8_tick_data_len();
    let mask = size - 1;
    let data = tick.u64data();
    let map = MAP.as_mut().unwrap();
    let array = find_array(map, quote_id, size);
    let slice = array.data.as_mut_slice();
    let base = array.walker;
    for i in 0..element_size {
        fill_u64(&mut slice[base + i * 8..base + (i + 1) * 8], data[i]);
    }
    array.walker = (array.walker + step) & mask;
}

#[inline]
fn find_array(
    map: &mut HashMap<u16, WrappedArray>,
    quote_id: u16,
    size: usize,
) -> &mut WrappedArray {
    let array = map
        .entry(quote_id)
        .or_insert_with(|| WrappedArray::new(vec![0u8; size]));
    array
}

struct WrappedArray {
    data: Vec<u8>,
    walker: usize,
}

impl WrappedArray {
    fn new(data: Vec<u8>) -> Self {
        WrappedArray { data, walker: 0 }
    }
}
