#![allow(dead_code)]

use std::{
    ptr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::consts::FIELD_SIZE;

const U16_FULL_VAL: u32 = u16::MAX as u32 + 1;

#[inline]
pub(crate) fn fill_ptr<T>(u8_ptr: *mut u8, data: T) {
    unsafe { *(u8_ptr as *mut T) = data };
}

#[inline]
pub(crate) fn fetch_ptr<T>(u8_ptr: *const u8) -> T
where
    T: Copy,
{
    unsafe { *(u8_ptr as *const T) }
}

#[inline]
pub(crate) fn fill<T>(slice: &mut [u8], data: T) {
    unsafe { ptr::write_unaligned(ptr::addr_of!(*slice) as *mut T, data) }
}

#[inline]
pub(crate) fn fetch<T>(slice: &[u8]) -> T
where
    T: Copy,
{
    unsafe { ptr::read_unaligned(ptr::addr_of!(*slice) as *mut T) }
}

#[inline]
pub(crate) fn bitmap_chk_id(array: &[u8], id: u16) -> bool {
    let (idx, bit) = fetch_idx_bit(id);
    array[idx as usize] & (1 << bit) == 0
}

#[inline]
pub(crate) fn bitmap_set_id(array: &mut [u8], id: u16) {
    let (idx, bit) = fetch_idx_bit(id);
    array[idx as usize] |= 1 << bit;
}

#[inline]
fn fetch_idx_bit(id: u16) -> (u16, u16) {
    let idx = id / (FIELD_SIZE as u16);
    let bit = id % (FIELD_SIZE as u16);
    (idx, bit)
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
    pub(crate) fn insert<T>(&mut self, id: u16, value: T) {
        let val = Box::new(value);
        let p_val = Box::leak(val);
        // let v_ptr = ptr::addr_of_mut!(*p_val) as u64;
        // log::info!("insert ptr: {v_ptr}");
        // self.ptr_array[id as usize] = v_ptr;
        self.ptr_array[id as usize] = ptr::addr_of_mut!(*p_val) as u64;
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
    pub(crate) fn get_mut<T>(&mut self, id: u16) -> Option<&'static mut T> {
        let u64v = self.ptr_array[id as usize];
        if u64v == 0 {
            None
        } else {
            // log::info!("{}, mut id: {id}, addr: {u64v}", ptr::addr_of!(self.ptr_array) as u64);
            let p_val = u64v as *mut T;
            unsafe { Some(&mut *p_val) }
        }
    }
    #[inline]
    pub(crate) fn get<T>(&self, id: u16) -> Option<&'static T> {
        let u64v = self.ptr_array[id as usize];
        if u64v == 0 {
            None
        } else {
            // log::info!("{}, id: {id}, addr: {u64v}", ptr::addr_of!(self.ptr_array) as u64);
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
                // log::info!("not exist, insert id: {id}");
                map.insert(id, f());
            }
        }
    }

    #[inline]
    pub(crate) fn _fetch_as_mut<T>(&self, map: &mut SimpleU16Map) -> Option<&'static mut T> {
        match *self {
            SimpleU16Entry::Exist(id) => map.get_mut(id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fetch, fetch_ptr, fill, fill_ptr};
    use crate::{consts::FIELD_SIZE, utest::base::test_init};
    use std::alloc::{self, Layout};

    #[test]
    fn test() {
        test_init();
        let mut array = [0_u8; FIELD_SIZE];
        let slice = array.as_mut_slice();
        let i64v = 1234;
        fill(slice, i64v);
        let fetched: i64 = fetch(slice);
        log::info!("{i64v},{fetched}");
        let f64v = 1234.5678;
        fill(slice, f64v);
        let fetched: f64 = fetch(slice);
        log::info!("{f64v},{fetched}");
    }

    #[test]
    fn test2() {
        test_init();
        let u8_ptr = unsafe { alloc::alloc(Layout::from_size_align_unchecked(8, 1)) };
        let u32v: u32 = 1234;
        fill_ptr(u8_ptr, u32v);
        let fetched: u32 = fetch_ptr(u8_ptr);
        log::info!("u32 {u32v},{fetched}");
        let i32v: i32 = 1234;
        fill_ptr(u8_ptr, i32v);
        let fetched: i32 = fetch_ptr(u8_ptr);
        log::info!("i32 {i32v},{fetched}");
        let f32v: f32 = 1234.5678;
        fill_ptr(u8_ptr, f32v);
        let fetched: f32 = fetch_ptr(u8_ptr);
        log::info!("f32 {f32v},{fetched}");
        let u64v: u64 = 1234;
        fill_ptr(u8_ptr, u64v);
        let fetched: u64 = fetch_ptr(u8_ptr);
        log::info!("u64 {u64v},{fetched}");
        let i64v: i64 = 1234;
        fill_ptr(u8_ptr, i64v);
        let fetched: i64 = fetch_ptr(u8_ptr);
        log::info!("i64 {i64v},{fetched}");
        let f64v: f64 = 1234.5678;
        fill_ptr(u8_ptr, f64v);
        let fetched: f64 = fetch_ptr(u8_ptr);
        log::info!("f64 {f64v},{fetched}");
    }
}
