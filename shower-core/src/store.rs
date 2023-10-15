use crate::{aux::SimpleU16Map, consts::U8_DATA_MAX_SIZE, data::U8Bytes};
use std::sync::Once;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<SimpleU16Map<WrappedArray>> = None;

pub(crate) fn insert(data: &U8Bytes) {
    MAP_INIT.call_once(initial);
    insert_into_slice(&data);
}

fn initial() {
    unsafe { MAP.get_or_insert(SimpleU16Map::new()) };
}

fn insert_into_slice(data: &U8Bytes) {
    let id = data.id();
    let size = data.len();
    let data = data.bytes();
    let map = unsafe { MAP.as_mut().unwrap() };
    let array = find_array(map, id, size);
    let base = array.walker();
    let slice = array.data();
    for i in 0..size {
        slice[base + i] = data[i];
    }
    array.update_walker(U8_DATA_MAX_SIZE);
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
    use crate::{aux::SimpleU16Map, utest_base::test_init, U8Bytes};

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
        for _ in 0..100 {
            super::insert(&U8Bytes::new_from_vec(16, 288, vec![0u8; 288]));
        }
    }
}
