use crate::aux::SimpleU16Map;
use std::sync::atomic::{AtomicU16, Ordering};

pub const LONG: Number = Number {
    _num_type: NumType::Long,
    _len: 8,
};
pub const DOUBLE: Number = Number {
    _num_type: NumType::Double,
    _len: 8,
};

static mut WALKER: Option<AtomicU16> = None;
static mut MAP: Option<SimpleU16Map<Vec<FieldDef>>> = None;

#[derive(Clone, Copy, Debug)]
pub enum NumType {
    Long,
    Double,
}

#[derive(Clone, Copy, Debug)]
pub struct Number {
    _num_type: NumType,
    _len: usize,
}
impl Number {
    pub(crate) fn _num_type(self) -> NumType {
        self._num_type
    }
    pub(crate) fn _len(self) -> usize {
        self._len
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FieldDef {
    Num(Number),
    Str(usize),
}

pub(crate) fn init_define_store() {
    unsafe {
        WALKER = Some(AtomicU16::new(0));
        MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn insert_define(defines: Vec<FieldDef>) -> u16 {
    unsafe {
        let map = MAP.as_mut().unwrap();
        let key = WALKER.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
        map.insert(key, defines);
        key
    }
}

pub(crate) fn get_field_define(id: u16, field_idx: u16) -> Option<FieldDef> {
    unsafe {
        let map = MAP.as_ref().unwrap();
        if let Some(defines) = map.get(id) {
            if field_idx as usize >= defines.len() {
                None
            } else {
                Some(defines[field_idx as usize].clone())
            }
        } else {
            None
        }
    }
}
