use crate::aux::SimpleU16Map;
use std::sync::atomic::{AtomicU16, Ordering};

static mut WALKER: Option<AtomicU16> = None;
static mut MAP: Option<SimpleU16Map<Vec<Column>>> = None;

#[derive(Clone, Copy, Debug)]
pub enum ColumnType {
    Long,
    Double,
    Str(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct Column {
    data_type: ColumnType,
    _idx: usize,
    offset: usize,
}
impl Column {
    pub fn new_long() -> Column {
        Column {
            data_type: ColumnType::Long,
            _idx: 0,
            offset: 0,
        }
    }
    pub fn new_double() -> Column {
        Column {
            data_type: ColumnType::Double,
            _idx: 0,
            offset: 0,
        }
    }
    pub fn new_string(len: usize) -> Column {
        Column {
            data_type: ColumnType::Str(len),
            _idx: 0,
            offset: 0,
        }
    }
    pub(crate) fn data_type(&self) -> &ColumnType {
        &self.data_type
    }
    pub(crate) fn _idx(&self) -> usize {
        self._idx
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
    pub(crate) fn _adjust(&mut self, idx: usize, offset: usize) {
        self._idx = idx;
        self.offset = offset;
    }
}

pub(crate) fn init_define_store() {
    unsafe {
        WALKER = Some(AtomicU16::new(1));
        MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn insert_define(defines: Vec<Column>) -> u16 {
    unsafe {
        let map = MAP.as_mut().unwrap();
        let key = WALKER.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
        map.insert(key, defines);
        key
    }
}

pub(crate) fn get_define<'a>(id: u16) -> Option<&'a Vec<Column>> {
    unsafe {
        let map = MAP.as_ref().unwrap();
        if let Some(defines) = map.get(id) {
            Some(defines)
        } else {
            None
        }
    }
}
