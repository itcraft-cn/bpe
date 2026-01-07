use crate::{
    aux::SimpleU16Map,
    cfg::get_config,
    consts::{
        DEFAULT_RECORD_SIZE, DEFAULT_VEC_SIZE, KEY_RECORD_SIZE, KEY_VEC_SIZE, U8_DATA_MAX_SIZE,
    },
    data::U8Bytes,
};
use globalvar::{def_global_ptr, get_global_mut};
use std::{
    alloc::{self, Layout},
    ptr,
};

static mut PTR_MAP: u64 = 0;
static mut VEC_SIZE: usize = 0;
static mut RECORD_SIZE: usize = 0;

pub(crate) fn init_store() {
    unsafe {
        PTR_MAP = def_global_ptr(SimpleU16Map::new());
        VEC_SIZE = get_config()
            .fetch_cfg_usize(KEY_VEC_SIZE)
            .unwrap_or(DEFAULT_VEC_SIZE);
        let record_size = get_config()
            .fetch_cfg_usize(KEY_RECORD_SIZE)
            .unwrap_or(DEFAULT_RECORD_SIZE);
        RECORD_SIZE = if record_size > U8_DATA_MAX_SIZE {
            U8_DATA_MAX_SIZE
        } else {
            record_size
        };
    }
}

pub(crate) fn insert(array: &mut WrappedArray, data: &U8Bytes) {
    insert_into_slice(array, data);
}

fn insert_into_slice(array: &mut WrappedArray, data: &U8Bytes) {
    let size = data.data_len();
    let data = data.bytes();
    let base = (array.walker() & array.mask()) * array.step();
    array.write_data(base, &data[0..size], size);
}

#[inline]
pub(crate) fn find_or_insert_array<'a>(id: u16) -> &'a mut WrappedArray {
    let map = get_global_mut::<SimpleU16Map>(unsafe { PTR_MAP });
    map.entry(id)
        .or_insert_with(map, || WrappedArray::new(get_vec_size(), get_record_size()));
    map.get_mut(id).unwrap()
}

pub(crate) fn get_vec_size() -> usize {
    unsafe { VEC_SIZE }
}

pub(crate) fn get_record_size() -> usize {
    unsafe { RECORD_SIZE }
}

#[derive(Debug)]
pub(crate) struct WrappedArray {
    data: *mut u8,
    max_records: usize,
    mask: usize,
    walker: usize,
    step: usize,
}

impl WrappedArray {
    fn new(size: usize, step: usize) -> Self {
        let layout = Layout::from_size_align(size, 1).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        WrappedArray {
            data: ptr,
            max_records: size / step,
            mask: size - 1,
            walker: 0,
            step,
        }
    }

    pub(crate) fn _size(&self) -> usize {
        let walker = self.walker / self.step;
        if walker >= self.max_records {
            self.max_records
        } else {
            walker
        }
    }

    pub(crate) fn first_idx(&self) -> usize {
        let walker = (self.walker / self.step) - 1;
        if walker > self.max_records {
            (walker - self.max_records) % self.max_records
        } else {
            0
        }
    }

    pub(crate) fn last_idx(&self) -> usize {
        ((self.walker / self.step) - 1) % self.max_records
    }

    pub(crate) fn mask(&self) -> usize {
        self.mask
    }

    pub(crate) fn step(&self) -> usize {
        self.step
    }

    pub(crate) fn u64ptr(&self) -> u64 {
        self.data as u64
    }

    fn write_data(&mut self, base: usize, src_data: &[u8], len: usize) {
        unsafe {
            ptr::copy_nonoverlapping(src_data.as_ptr(), self.data.add(base & self.mask), len)
        };
        self.update_walker();
    }

    pub(crate) fn sub_data(&self, offset: usize) -> *const u8 {
        unsafe { self.data.add(offset) }
    }

    pub(crate) fn walker(&self) -> usize {
        self.walker / self.step
    }

    fn update_walker(&mut self) {
        self.walker += self.step;
    }
}

#[cfg(test)]
mod tests {
    use super::{insert, WrappedArray};
    use crate::{aux::SimpleU16Map, consts::U8_DATA_MAX_SIZE, utest::base::test_init, U8Bytes};

    #[test]
    fn test() {
        test_init();
        let mut map = SimpleU16Map::new();
        map.entry(1)
            .or_insert_with(&mut map, || WrappedArray::new(U8_DATA_MAX_SIZE * 1024, U8_DATA_MAX_SIZE));
        if let Some(array) = map.get_mut::<WrappedArray>(1) {
            log::info!("{:?}", array.walker());
            for _ in 0..100 {
                insert(array, &U8Bytes::new_from_vec(16, 288, vec![0_u8; 288]));
            }
        }
    }
}
