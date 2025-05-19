use crate::{
    aux::SimpleU16Map,
    cfg::get_config,
    consts::{DEFAULT_VEC_SIZE, KEY_VEC_SIZE, U8_DATA_MAX_SIZE},
    data::U8Bytes,
};
use globalvar::{def_global_ptr, get_global_mut};
use std::{
    alloc::{self, Layout},
    ptr,
};

static mut PTR_MAP: u64 = 0;
static mut VEC_SIZE: usize = 0;

pub(crate) fn init_store() {
    unsafe {
        PTR_MAP = def_global_ptr(SimpleU16Map::new());
        VEC_SIZE = get_config()
            .fetch_cfg_usize(KEY_VEC_SIZE)
            .unwrap_or(DEFAULT_VEC_SIZE);
    }
}

pub(crate) fn insert(array: &mut WrappedArray, data: &U8Bytes) {
    insert_into_slice(array, data);
}

fn insert_into_slice(array: &mut WrappedArray, data: &U8Bytes) {
    let size = data.data_len();
    let data = data.bytes();
    let base = array.walker() & array.mask();
    array.write_data(base, &data[0..size], size);
}

#[inline]
pub(crate) fn find_or_insert_array<'a>(id: u16) -> &'a mut WrappedArray {
    let map = get_global_mut::<SimpleU16Map>(unsafe { PTR_MAP });
    map
        .entry(id)
        .or_insert_with(map, || WrappedArray::new(get_vec_size()));
    map.get_mut(id).unwrap()
}

pub(crate) fn get_vec_size() -> usize {
    unsafe { VEC_SIZE }
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

    pub(crate) fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn mask(&self) -> usize {
        self.mask
    }

    pub(crate) fn u64ptr(&self) -> u64 {
        self.data as u64
    }

    fn write_data(&mut self, base: usize, src_data: &[u8], len: usize) {
        let src_ptr = src_data.as_ptr();
        unsafe { ptr::copy_nonoverlapping(src_ptr, self.data.add(base), len) };
        self.update_walker(U8_DATA_MAX_SIZE);
    }

    pub(crate) fn sub_data(&self, offset: usize) -> *const u8 {
        unsafe { self.data.add(offset) }
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
        if let Some(array) = map.get_mut::<WrappedArray>(1) {
            log::info!("{:?}", array.walker());
            for _ in 0..100 {
                insert(array, &U8Bytes::new_from_vec(16, 288, vec![0_u8; 288]));
            }
        }
    }
}
