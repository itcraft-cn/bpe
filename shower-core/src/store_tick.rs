use crate::{
    aux::{fill_u64, SimpleU16Map, WrappedArray},
    data::U8Tick,
};
use std::sync::Once;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<SimpleU16Map> = None;

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
    let mask = size - 1;
    let data = tick.u64data();
    let map = unsafe { MAP.as_mut().unwrap() };
    let array = find_array(map, quote_id, size);
    let base = array.walker();
    let slice = array.data();
    for i in 0..element_size {
        fill_u64(&mut slice[base + i * 8..base + (i + 1) * 8], data[i]);
    }
    array.update_walker(step, mask);
}

#[inline]
fn find_array(map: &mut SimpleU16Map, quote_id: u16, size: usize) -> &mut WrappedArray {
    map.entry(quote_id)
        .or_insert_with(map, || WrappedArray::new(vec![0u8; size]));
    map.get_mut(quote_id)
}

#[cfg(test)]
mod tests {
    use crate::{
        aux::{SimpleU16Map, WrappedArray},
        data::{TickConvU8, TICK_SIZE},
        utest_base::test_init,
        Tick,
    };

    #[test]
    fn test() {
        test_init();
        let mut map = SimpleU16Map::new();
        map.entry(1)
            .or_insert_with(&mut map, || WrappedArray::new(vec![0u8; 0]));
        let x = map.get_mut(1);
        log::info!("{:?}", x.walker());
    }

    #[test]
    fn test2() {
        test_init();
        let tick = Tick::new(1, 1, 1, 1, 1, 1);
        for _ in 0..100 {
            super::insert(&tick.convert(TICK_SIZE * 65536));
        }
    }
}
