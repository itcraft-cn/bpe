use crate::{aux::SimpleU16Map, consts::U8_DATA_MAX_SIZE};
use std::{
    cmp::Ordering as CmpOrdering,
    sync::atomic::{AtomicU16, Ordering},
};

static mut WALKER: Option<AtomicU16> = None;
static mut MAP: Option<SimpleU16Map<Vec<Column>>> = None;

#[derive(Clone, Copy, Debug)]
pub struct U8Bytes {
    id: u16,
    data_len: usize,
    bytes: [u8; U8_DATA_MAX_SIZE],
}
impl U8Bytes {
    pub fn new(id: u16, data_len: usize, bytes: [u8; U8_DATA_MAX_SIZE]) -> U8Bytes {
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn new_from_vec(id: u16, data_len: usize, vec: Vec<u8>) -> U8Bytes {
        let mut bytes = [0u8; U8_DATA_MAX_SIZE];
        let slice = vec.as_slice();
        match slice.len().cmp(&U8_DATA_MAX_SIZE) {
            CmpOrdering::Less => copy(slice.len(), &mut bytes, slice),
            CmpOrdering::Equal | CmpOrdering::Greater => copy(U8_DATA_MAX_SIZE, &mut bytes, slice),
        }
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn new_from_slice(id: u16, data_len: usize, slice: &[u8]) -> U8Bytes {
        let mut bytes = [0u8; U8_DATA_MAX_SIZE];
        match slice.len().cmp(&U8_DATA_MAX_SIZE) {
            CmpOrdering::Less => copy(slice.len(), &mut bytes, slice),
            CmpOrdering::Equal | CmpOrdering::Greater => copy(U8_DATA_MAX_SIZE, &mut bytes, slice),
        }
        U8Bytes {
            id,
            data_len,
            bytes,
        }
    }
    pub fn id(&self) -> u16 {
        self.id
    }
    pub fn data_len(&self) -> usize {
        self.data_len
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

fn copy(len: usize, bytes: &mut [u8; U8_DATA_MAX_SIZE], slice: &[u8]) {
    bytes.as_mut_slice()[0..len].copy_from_slice(&slice[0..len]);
}

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

    fn copy_from_columns(columns: &Vec<Column>) -> Vec<Column> {
        let mut target = vec![];
        let mut offset = 0usize;
        for col_with_id in columns.iter().enumerate() {
            let mut column = col_with_id.1.clone();
            column._idx = col_with_id.0;
            column.offset = offset;
            offset += match column.data_type {
                ColumnType::Long => 8,
                ColumnType::Double => 8,
                ColumnType::Str(len) => len,
            };
            target.push(column);
        }
        target
    }
}

pub(crate) fn init_record_store() {
    unsafe {
        WALKER = Some(AtomicU16::new(1));
        MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn insert_record(columns: Vec<Column>) -> u16 {
    unsafe {
        let map = MAP.as_mut().unwrap();
        let key = WALKER.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
        map.insert(key, Column::copy_from_columns(&columns));
        key
    }
}

pub(crate) fn get_column<'a>(id: u16) -> Option<&'a Vec<Column>> {
    let map = unsafe { MAP.as_ref().unwrap() };
    map.get(id)
}
