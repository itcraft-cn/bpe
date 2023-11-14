use crate::{aux::SimpleU16Map, consts::U8_DATA_MAX_SIZE};
use std::{
    cmp::Ordering as CmpOrdering,
    sync::atomic::{AtomicU16, Ordering},
};

static mut WALKER: Option<AtomicU16> = None;
static mut MAP: Option<SimpleU16Map<Record>> = None;

static mut ID_STORE: [u8; 8192] = [0; 8192];

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

#[derive(Clone, Copy, Debug)]
pub(crate) enum RecordType {
    Incoming,
    Stream,
}

#[derive(Clone, Debug)]
pub(crate) struct Record {
    _record_type: RecordType,
    columns: Vec<Column>,
}
impl Record {
    fn new(record_type: RecordType, columns: Vec<Column>) -> Self {
        Record {
            _record_type: record_type,
            columns,
        }
    }
    pub(crate) fn _record_type(&self) -> &RecordType {
        &self._record_type
    }
    pub(crate) fn columns(&self) -> &Vec<Column> {
        &self.columns
    }
}

pub(crate) fn init_record_store() {
    unsafe {
        WALKER = Some(AtomicU16::new(1));
        MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn insert_record(record_type: RecordType, columns: Vec<Column>) -> u16 {
    unsafe {
        let map = MAP.as_mut().unwrap();
        let key = WALKER.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
        map.insert(
            key,
            Record::new(record_type, Column::copy_from_columns(&columns)),
        );
        let (idx, bit) = fetch_idx_bit(key);
        ID_STORE[idx as usize] |= 1 << bit;
        key
    }
}

pub(crate) fn get_column<'a>(id: u16) -> Option<&'a Record> {
    let map = unsafe { MAP.as_ref().unwrap() };
    map.get(id)
}

pub(crate) fn check_id(id: u16) -> bool {
    let (idx, bit) = fetch_idx_bit(id);
    unsafe { ID_STORE[idx as usize] & (1 << bit) == 0 }
}

fn fetch_idx_bit(id: u16) -> (u16, u16) {
    let idx = id / 8;
    let bit = id % 8;
    (idx, bit)
}
