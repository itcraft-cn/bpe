use crate::{aux::fill_u64, data::U8Tick};
use std::sync::Once;

const U16_FULL_VAL: u32 = u16::MAX as u32 + 1;

static MAP_INIT: Once = Once::new();
static mut MAP: Option<SimpleU16Map> = None;

pub(crate) fn insert(tick: U8Tick) {
    MAP_INIT.call_once(|| unsafe { initial() });
    unsafe {
        insert_into_slice(tick);
    }
}

unsafe fn initial() {
    MAP.get_or_insert(SimpleU16Map::new());
}

unsafe fn insert_into_slice(tick: U8Tick) {
    let quote_id = tick.quote_id();
    let element_size = tick.element_size();
    let step = tick.tick_size();
    let size = tick.u8_tick_data_len();
    let mask = size - 1;
    let data = tick.u64data();
    let map = MAP.as_mut().unwrap();
    let array = find_array(map, quote_id, size);
    let slice = array.data.as_mut_slice();
    let base = array.walker;
    for i in 0..element_size {
        fill_u64(&mut slice[base + i * 8..base + (i + 1) * 8], data[i]);
    }
    array.walker = (array.walker + step) & mask;
}

#[inline]
fn find_array(map: &mut SimpleU16Map, quote_id: u16, size: usize) -> &mut WrappedArray {
    map.entry(quote_id)
        .or_insert_with(map, || WrappedArray::new(vec![0u8; size]));
    map.get_mut(quote_id)
}

struct WrappedArray {
    data: Vec<u8>,
    walker: usize,
}

impl WrappedArray {
    fn new(data: Vec<u8>) -> Self {
        WrappedArray { data, walker: 0 }
    }
}

pub(crate) struct SimpleU16Map {
    vec: Vec<Option<WrappedArray>>,
}
impl SimpleU16Map {
    fn new() -> Self {
        let mut vec = vec![];
        for _ in 0..U16_FULL_VAL {
            vec.push(None);
        }
        SimpleU16Map { vec }
    }
    #[inline]
    fn entry(&mut self, id: u16) -> SimpleU16Entry {
        let opt = self.vec[id as usize].as_mut();
        match opt {
            Some(_) => SimpleU16Entry::Exist(id),
            None => SimpleU16Entry::NotExist(id),
        }
    }
    #[inline]
    fn get_mut(&mut self, id: u16) -> &mut WrappedArray {
        self.vec[id as usize].as_mut().unwrap()
    }
}

enum SimpleU16Entry {
    Exist(u16),
    NotExist(u16),
}
impl SimpleU16Entry {
    #[inline]
    fn or_insert_with<F>(&mut self, map: &mut SimpleU16Map, f: F)
    where
        F: FnOnce() -> WrappedArray,
    {
        match *self {
            SimpleU16Entry::Exist(_) => (),
            SimpleU16Entry::NotExist(id) => {
                map.vec[id as usize] = Some(f());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{cfg, consts::SHOWER_ENV_HOME_KEY, data::TICK_SIZE, logger, Tick};
    use std::env;

    //#[test]
    fn _test() {
        env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
        cfg::load_config();
        logger::init_logger(cfg::get_config());
        let mut map = super::SimpleU16Map::new();
        map.entry(1)
            .or_insert_with(&mut map, || super::WrappedArray {
                data: vec![0u8; 0],
                walker: 0,
            });
        let x = map.get_mut(1);
        log::info!("{:?}", x.walker);
    }

    #[test]
    fn _test2() {
        env::set_var(SHOWER_ENV_HOME_KEY, "/home/helly/code/rust/shower");
        cfg::load_config();
        logger::init_logger(cfg::get_config());
        let tick = Tick::new(1, 1, 1, 1, 1, 1);
        for _ in 0..100000000 {
            super::insert(tick.convert(TICK_SIZE * 65536));
        }
    }
}
