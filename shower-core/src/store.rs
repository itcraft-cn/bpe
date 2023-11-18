use crate::{
    aux::SimpleU16Map,
    cfg::get_config,
    consts::{DEFAULT_VEC_SIZE, KEY_VEC_SIZE, U8_DATA_MAX_SIZE},
    data::U8Bytes,
};
use std::{
    alloc::{self, Layout},
    slice,
    sync::Once,
};

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
    let array = find_or_insert_array_mut(id);
    let base = array.walker() & array.mask();
    let slice = array.data();
    slice[base..base + size].copy_from_slice(&data[0..size]);
    array.update_walker(U8_DATA_MAX_SIZE);
}

#[inline]
fn find_or_insert_array_mut<'a>(id: u16) -> &'a mut WrappedArray {
    let map = unsafe { MAP.as_mut().unwrap() };
    map.entry(id)
        .or_insert_with(map, || WrappedArray::new(unsafe { VEC_SIZE }));
    map.get_mut(id).unwrap()
}

#[inline]
pub(crate) fn find_or_insert_array<'a>(id: u16) -> &'a WrappedArray {
    find_or_insert_array_mut(id)
}

#[derive(Debug)]
pub(crate) struct WrappedArray {
    data: *mut u8,
    size: usize,
    mask: usize,
    walker: usize,
}

impl WrappedArray {
    fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        WrappedArray {
            data: ptr,
            size,
            mask: size - 1,
            walker: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        if self.walker > self.mask {
            self.size
        } else {
            self.walker
        }
    }

    pub(crate) fn records(&self) -> usize {
        self.len() / U8_DATA_MAX_SIZE
    }

    fn _size(&self) -> usize {
        self.size
    }

    pub(crate) fn mask(&self) -> usize {
        self.mask
    }

    fn data(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.data, self.size) }
    }

    pub(crate) fn sub_data(&self, offset: usize) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(
                (self.data as u64 + offset as u64) as *mut u8,
                U8_DATA_MAX_SIZE,
            )
        }
    }

    pub(crate) fn walker(&self) -> usize {
        self.walker
    }

    fn update_walker(&mut self, step: usize) {
        self.walker += step;
    }
}

#[cfg(test)]
mod tests {
    use super::{insert, WrappedArray};
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
}
