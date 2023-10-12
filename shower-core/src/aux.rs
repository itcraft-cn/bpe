use std::time::{Duration, SystemTime, UNIX_EPOCH};

const U16_FULL_VAL: u32 = u16::MAX as u32 + 1;

#[inline]
pub(crate) fn fill_u64(slice: &mut [u8], data: u64) {
    slice[0] = data as u8;
    slice[1] = (data >> 1) as u8;
    slice[2] = (data >> 2) as u8;
    slice[3] = (data >> 3) as u8;
    slice[4] = (data >> 4) as u8;
    slice[5] = (data >> 5) as u8;
    slice[6] = (data >> 6) as u8;
    slice[7] = (data >> 7) as u8;
}

#[inline]
pub(crate) fn _fetch_u64(slice: &[u8]) -> u64 {
    (slice[0] as u64)
        | ((slice[1] as u64) << 1)
        | ((slice[2] as u64) << 2)
        | ((slice[3] as u64) << 3)
        | ((slice[4] as u64) << 4)
        | ((slice[5] as u64) << 5)
        | ((slice[6] as u64) << 6)
        | ((slice[7] as u64) << 7)
}

#[inline]
pub(crate) fn _timestamp() -> u64 {
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
}
