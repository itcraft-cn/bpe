#![allow(dead_code)]

use std::{
    ptr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const U16_FULL_VAL: u32 = u16::MAX as u32 + 1;

#[inline]
fn fill<T>(slice: &mut [u8], data: T) {
    let p_val = ptr::addr_of!(*slice);
    let p_data = p_val as *mut T;
    unsafe { *p_data = data };
}

#[inline]
fn fetch<T>(slice: &[u8]) -> T
where
    T: Copy,
{
    let p_val = ptr::addr_of!(*slice);
    let p_data = p_val as *const T;
    unsafe { *p_data }
}

#[inline]
pub(crate) fn fill_i64(slice: &mut [u8], data: i64) {
    fill(slice, data)
}

#[inline]
pub(crate) fn fetch_i64(slice: &[u8]) -> i64 {
    fetch(slice)
}

#[inline]
pub(crate) fn fill_f64(slice: &mut [u8], data: f64) {
    fill(slice, data)
}

#[inline]
pub(crate) fn fetch_f64(slice: &[u8]) -> f64 {
    fetch(slice)
}

#[inline]
pub(crate) fn timestamp() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    duration.as_millis() as u64
}

pub(crate) struct SimpleU16Map {
    ptr_array: [u64; U16_FULL_VAL as usize],
}
impl SimpleU16Map {
    pub(crate) fn new() -> Self {
        SimpleU16Map {
            ptr_array: [0_u64; U16_FULL_VAL as usize],
        }
    }
    #[inline]
    pub(crate) fn insert<T>(&mut self, key: u16, value: T) {
        let val = Box::new(value);
        let p_val = Box::leak(val);
        self.ptr_array[key as usize] = ptr::addr_of_mut!(*p_val) as u64;
    }
    #[inline]
    pub(crate) fn entry(&mut self, id: u16) -> SimpleU16Entry {
        let ptr = self.ptr_array[id as usize];
        if ptr == 0 {
            SimpleU16Entry::NotExist(id)
        } else {
            SimpleU16Entry::Exist(id)
        }
    }
    #[inline]
    pub(crate) fn get_mut<'a, T>(&'a mut self, id: u16) -> Option<&'a mut T> {
        let u64v = self.ptr_array[id as usize];
        if u64v == 0 {
            None
        } else {
            let p_val = u64v as *mut T;
            unsafe { Some(&mut *p_val) }
        }
    }
    #[inline]
    pub(crate) fn get<T>(&self, id: u16) -> Option<&T> {
        let u64v = self.ptr_array[id as usize];
        if u64v == 0 {
            None
        } else {
            let p_val = u64v as *mut T;
            unsafe { Some(&*p_val) }
        }
    }
}

pub(crate) enum SimpleU16Entry {
    Exist(u16),
    NotExist(u16),
}
impl SimpleU16Entry {
    #[inline]
    pub(crate) fn or_insert_with<T, F>(&mut self, map: &mut SimpleU16Map, f: F)
    where
        F: FnOnce() -> T,
    {
        match *self {
            SimpleU16Entry::Exist(_) => (),
            SimpleU16Entry::NotExist(id) => {
                map.insert(id, f());
            }
        }
    }

    #[inline]
    pub(crate) fn _fetch_as_mut<'a, T>(&self, map: &'a mut SimpleU16Map) -> Option<&'a mut T> {
        match *self {
            SimpleU16Entry::Exist(id) => map.get_mut(id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fetch_f64, fetch_i64, fill_f64, fill_i64};
    use crate::utest::base::test_init;

    #[test]
    fn test() {
        test_init();
        let mut array = [0_u8; 8];
        let slice = array.as_mut_slice();
        let i64v = 1234;
        fill_i64(slice, i64v);
        let fetched = fetch_i64(slice);
        log::info!("{},{}", i64v, fetched);
        let f64v = 1234.5678;
        fill_f64(slice, f64v);
        let fetched = fetch_f64(slice);
        log::info!("{},{}", f64v, fetched);
    }
}
