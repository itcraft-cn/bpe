#![allow(dead_code)]

use std::{
    ptr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const U16_FULL_VAL: u32 = u16::MAX as u32 + 1;

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *mut u64;
    unsafe { *p_u64 = data };
}

#[inline]
pub(crate) fn fetch_u64(slice: &[u8]) -> u64 {
    let p_val = ptr::addr_of!(*slice);
    let p_u64 = p_val as *const u64;
    unsafe { *p_u64 }
}

#[inline]
pub(crate) fn fill_f64(slice: &mut [u8], data: f64) {
    fill_u64(slice, data.to_bits());
}

#[inline]
pub(crate) fn fetch_f64(slice: &[u8]) -> f64 {
    f64::from_bits(fetch_u64(slice))
}

#[inline]
pub(crate) fn timestamp() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_e| Duration::new(0, 0));
    duration.as_millis() as u64
}

pub(crate) struct SimpleU16Map<T> {
    vec: Vec<Option<T>>,
}
impl<T> SimpleU16Map<T> {
    pub(crate) fn new() -> Self {
        let mut vec = vec![];
        for _ in 0..U16_FULL_VAL {
            vec.push(None);
        }
        SimpleU16Map { vec }
    }
    #[inline]
    pub(crate) fn insert(&mut self, key: u16, value: T) {
        self.vec[key as usize] = Some(value);
    }
    #[inline]
    pub(crate) fn entry(&mut self, id: u16) -> SimpleU16Entry {
        let opt = self.vec[id as usize].as_mut();
        match opt {
            Some(_) => SimpleU16Entry::Exist(id),
            None => SimpleU16Entry::NotExist(id),
        }
    }
    #[inline]
    pub(crate) fn get_mut(&mut self, id: u16) -> &mut T {
        self.vec[id as usize].as_mut().unwrap()
    }
    #[inline]
    pub(crate) fn get(&self, id: u16) -> Option<&T> {
        self.vec[id as usize].as_ref()
    }
}

pub(crate) enum SimpleU16Entry {
    Exist(u16),
    NotExist(u16),
}
impl SimpleU16Entry {
    #[inline]
    pub(crate) fn or_insert_with<T, F>(&mut self, map: &mut SimpleU16Map<T>, f: F)
    where
        F: FnOnce() -> T,
    {
        match *self {
            SimpleU16Entry::Exist(_) => (),
            SimpleU16Entry::NotExist(id) => {
                map.vec[id as usize] = Some(f());
            }
        }
    }

    #[inline]
    pub(crate) fn _fetch_as_mut<'a, T>(&self, map: &'a mut SimpleU16Map<T>) -> Option<&'a mut T> {
        match *self {
            SimpleU16Entry::Exist(id) => Some(map.get_mut(id)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fetch_f64, fetch_u64, fill_f64, fill_u64};
    use crate::utest_base::test_init;

    #[test]
    fn test() {
        test_init();
        log::info!("max u64: {}", u64::MAX);
        let mut array = [0u8; 8];
        let slice = array.as_mut_slice();
        let u64v = 1234;
        fill_u64(slice, u64v);
        let fetched = fetch_u64(slice);
        log::info!("{},{}", u64v, fetched);
        let f64v = 1234.5678;
        fill_f64(slice, f64v);
        let fetched = fetch_f64(slice);
        log::info!("{},{}", f64v, fetched);
    }
}
