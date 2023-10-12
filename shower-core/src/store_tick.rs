use crate::{
    aux::{fill_u64, SimpleU16Map},
    data::U8Tick,
};
use std::sync::Once;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<SimpleU16Map<WrappedArray>> = None;

pub(crate) fn insert(tick: &U8Tick) {
    MAP_INIT.call_once(initial);
    insert_into_slice(&tick);
}

fn initial() {
    unsafe { MAP.get_or_insert(SimpleU16Map::new()) };
}

fn insert_into_slice(tick: &U8Tick) {
    let quote_id = tick.quote_id();
    let element_size = tick.element_size();
    let step = tick.tick_size();
    let size = tick.u8_tick_data_len();
    let data = tick.u64data();
    let map = unsafe { MAP.as_mut().unwrap() };
    let array = find_array(map, quote_id, size);
    let base = array.walker();
    let slice = array.data();
    for i in 0..element_size {
        fill_u64(&mut slice[base + i * 8..base + (i + 1) * 8], data[i]);
    }
    array.update_walker(step);
}

#[inline]
fn find_array(
    map: &mut SimpleU16Map<WrappedArray>,
    quote_id: u16,
    size: usize,
) -> &mut WrappedArray {
    map.entry(quote_id)
        .or_insert_with(map, || WrappedArray::new(size));
    map.get_mut(quote_id)
}

struct WrappedArray {
    data: Vec<u8>,
    mask: usize,
    walker: usize,
}

impl WrappedArray {
    fn new(size: usize) -> Self {
        WrappedArray {
            data: vec![0u8; size],
            mask: size - 1,
            walker: 0,
        }
    }
    fn data(&mut self) -> &mut [u8] {
        let slice = self.data.as_mut_slice();
        slice
    }
    fn walker(&self) -> usize {
        self.walker
    }

    fn update_walker(&mut self, step: usize) {
        self.walker = (self.walker + step) & self.mask;
    }
}

#[cfg(test)]
mod tests {
    use super::WrappedArray;
    use crate::{
        aux::{SimpleU16Map, _timestamp},
        data::{TickConvU8, TICK_SIZE},
        utest_base::test_init,
        Tick,
    };

    #[test]
    fn test() {
        test_init();
        let mut map: SimpleU16Map<WrappedArray> = SimpleU16Map::new();
        map.entry(1)
            .or_insert_with(&mut map, || WrappedArray::new(1));
        let x = map.get_mut(1);
        log::info!("{:?}", x.walker());
    }

    #[test]
    fn test2() {
        test_init();
        let tick = Tick::new(1, 1, 1, 1, 1, _timestamp());
        for _ in 0..100 {
            super::insert(&tick.convert(TICK_SIZE * 65536));
        }
    }
}
