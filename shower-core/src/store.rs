use crate::{
    aux::SimpleU16Map,
    cfg::get_config,
    consts::{DEFAULT_VEC_SIZE, KEY_VEC_SIZE, U8_DATA_MAX_SIZE},
    data::U8Bytes,
};
use std::sync::Once;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<SimpleU16Map> = None;
static mut VEC_SIZE: usize = 0;

pub(crate) fn insert(data: &U8Bytes) {
    MAP_INIT.call_once(initial);
    insert_into_slice(data);
}

fn initial() {
    unsafe {
        MAP.get_or_insert(SimpleU16Map::new());
        VEC_SIZE = get_config()
            .fetch_cfg_usize(KEY_VEC_SIZE)
            .unwrap_or(DEFAULT_VEC_SIZE);
    }
}

fn insert_into_slice(data: &U8Bytes) {
    let id = data.id();
    let size = data.data_len();
    let data = data.bytes();
    let array = find_or_insert_array(id);
    let base = array.walker() & array.mask();
    let slice = array.data();
    slice[base..base + size].copy_from_slice(&data[0..size]);
    array.update_walker(U8_DATA_MAX_SIZE);
}

#[inline]
fn find_or_insert_array<'a>(id: u16) -> &'a mut WrappedArray {
    let map = unsafe { MAP.as_mut().unwrap() };
    map.entry(id)
        .or_insert_with(map, || WrappedArray::new(unsafe { VEC_SIZE }));
    map.get_mut(id).unwrap()
}

pub(crate) fn create_iterator<'a>(id: u16) -> DataIterator<'a> {
    MAP_INIT.call_once(initial);
    DataIterator {
        array: find_or_insert_array(id),
        len: 0,
        offset: 0,
        first: true,
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct DataIterator<'a> {
    array: &'a WrappedArray,
    len: usize,
    offset: usize,
    first: bool,
}
impl<'a> Iterator for DataIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let walker = self.array.walker;
        if walker == 0 {
            return None;
        }
        if self.first {
            self.len = self.array.records();
            self.offset = 0;
            self.first = false;
        }
        let mask = self.array.mask;
        if self.offset == self.len {
            None
        } else {
            let position = (walker - U8_DATA_MAX_SIZE - self.offset * U8_DATA_MAX_SIZE) & mask;
            self.offset += 1;
            Some(&self.array.data.as_slice()[position..position + U8_DATA_MAX_SIZE])
        }
    }
}

#[derive(Debug)]
struct WrappedArray {
    data: Vec<u8>,
    size: usize,
    mask: usize,
    walker: usize,
}

impl WrappedArray {
    fn new(size: usize) -> Self {
        WrappedArray {
            data: vec![0_u8; size],
            size,
            mask: size - 1,
            walker: 0,
        }
    }

    fn len(&self) -> usize {
        if self.walker > self.mask {
            self.size
        } else {
            self.walker
        }
    }

    fn records(&self) -> usize {
        self.len() / U8_DATA_MAX_SIZE
    }

    fn _size(&self) -> usize {
        self.size
    }

    fn mask(&self) -> usize {
        self.mask
    }

    fn data(&mut self) -> &mut [u8] {
        let slice = self.data.as_mut_slice();
        slice
    }

    fn walker(&self) -> usize {
        self.walker
    }

    fn update_walker(&mut self, step: usize) {
        self.walker += step;
    }
}

#[cfg(test)]
mod tests {
    use super::{create_iterator, insert, WrappedArray};
    use crate::{aux::SimpleU16Map, utest::base::test_init, U8Bytes};

    #[test]
    fn test() {
        test_init();
        let mut map = SimpleU16Map::new();
        map.entry(1)
            .or_insert_with(&mut map, || WrappedArray::new(1));
        if let Some(x) = map.get_mut::<WrappedArray>(1) {
            log::info!("{:?}", x.walker());
        }
    }

    #[test]
    fn test2() {
        test_init();
        for _ in 0..100 {
            insert(&U8Bytes::new_from_vec(16, 288, vec![0_u8; 288]));
        }
    }

    #[test]
    fn test3() {
        test_init();
        for _ in 0..100 {
            insert(&U8Bytes::new_from_vec(32, 288, vec![0_u8; 288]));
        }
        let iterator = create_iterator(32);
        iterator.for_each(|slice| log::info!("p[{:?}]->[u8; {}]", slice.as_ptr(), slice.len()));
    }
}
