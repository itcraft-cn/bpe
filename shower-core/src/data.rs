use crate::{
    aux::{check_id, set_id, SimpleU16Map},
    consts::U8_DATA_MAX_SIZE,
    id::next_record_id,
};
use hashbrown::HashMap;
use std::cmp::Ordering as CmpOrdering;

static mut RECORD_MAP: Option<SimpleU16Map> = None;
static mut NAME_MAP: Option<HashMap<String, u16>> = None;

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
        let mut bytes = [0_u8; U8_DATA_MAX_SIZE];
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
        let mut bytes = [0_u8; U8_DATA_MAX_SIZE];
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
impl ColumnType {
    fn by_idx(type_idx: u16, size: usize) -> ColumnType {
        match type_idx {
            0 => ColumnType::Long,
            1 => ColumnType::Double,
            2 => ColumnType::Str(size),
            _ => ColumnType::Long,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Column {
    name: String,
    data_type: ColumnType,
    idx: u16,
    offset: usize,
}
impl Column {
    pub fn new_long(name: &str) -> Column {
        Column {
            name: String::from(name),
            data_type: ColumnType::Long,
            idx: 0,
            offset: 0,
        }
    }
    pub fn new_double(name: &str) -> Column {
        Column {
            name: String::from(name),
            data_type: ColumnType::Double,
            idx: 0,
            offset: 0,
        }
    }
    pub fn new_string(name: &str, len: usize) -> Column {
        Column {
            name: String::from(name),
            data_type: ColumnType::Str(len),
            idx: 0,
            offset: 0,
        }
    }
    pub fn new(name: String, type_idx: u16, size: usize) -> Column {
        Column {
            name,
            data_type: ColumnType::by_idx(type_idx, size),
            idx: 0,
            offset: 0,
        }
    }
    pub(crate) fn _name(&self) -> &str {
        &self.name
    }
    pub(crate) fn data_type(&self) -> &ColumnType {
        &self.data_type
    }
    pub(crate) fn _idx(&self) -> u16 {
        self.idx
    }
    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
    pub(crate) fn _adjust(&mut self, idx: u16, offset: usize) {
        self.idx = idx;
        self.offset = offset;
    }

    fn copy_from_columns(columns: &[Column]) -> Vec<Column> {
        let mut target = vec![];
        let mut offset = 0_usize;
        for col_with_id in columns.iter().enumerate() {
            let mut column = col_with_id.1.clone();
            column.idx = col_with_id.0 as u16 + 1;
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
    _name: String,
    id: u16,
    _record_type: RecordType,
    columns: Vec<Column>,
    columns_map: HashMap<String, u16>,
}
impl Record {
    fn new(name: &str, id: u16, record_type: RecordType, columns: Vec<Column>) -> Self {
        let mut map = HashMap::new();
        for column in &columns {
            map.insert(column.name.clone(), column.idx);
        }
        Record {
            _name: String::from(name),
            id,
            _record_type: record_type,
            columns,
            columns_map: map,
        }
    }

    pub(crate) fn insert_record(
        name: &str,
        record_type: RecordType,
        columns: Vec<Column>,
    ) -> Option<u16> {
        unsafe {
            let record_map = RECORD_MAP.as_mut().unwrap();
            let id = next_record_id();
            record_map.insert(
                id,
                Record::new(name, id, record_type, Column::copy_from_columns(&columns)),
            );
            let name_map = NAME_MAP.as_mut().unwrap();
            let key = String::from(name);
            if name_map.contains_key(&key) {
                log::warn!("Record name {} already exists", name);
                None
            } else {
                name_map.insert(key, id);
                set_id(ID_STORE.as_mut_slice(), id);
                Some(id)
            }
        }
    }
    pub(crate) fn get_record<'a>(id: u16) -> Option<&'a Record> {
        let map = unsafe { RECORD_MAP.as_ref().unwrap() };
        map.get(id)
    }
    pub(crate) fn fetch_record_id(name: &str) -> Option<&u16> {
        unsafe {
            let name_map = NAME_MAP.as_ref().unwrap();
            name_map.get(&String::from(name))
        }
    }
    pub(crate) fn fetch_field_id(record_id: u16, column_name: &str) -> Option<&u16> {
        if let Some(record) = Record::get_record(record_id) {
            record.column_id(column_name)
        } else {
            None
        }
    }
    pub(crate) fn _get_column(record_id: u16, column_id: u16) -> Option<&'static Column> {
        if let Some(record) = Record::get_record(record_id) {
            record.column(column_id)
        } else {
            None
        }
    }
    pub(crate) fn id(&self) -> u16 {
        self.id
    }
    pub(crate) fn _record_type(&self) -> &RecordType {
        &self._record_type
    }
    pub(crate) fn columns(&self) -> &Vec<Column> {
        &self.columns
    }
    pub(crate) fn column_id(&self, column_name: &str) -> Option<&u16> {
        self.columns_map.get(&String::from(column_name))
    }
    pub(crate) fn column(&self, column_id: u16) -> Option<&Column> {
        unsafe { Some(self.columns().get_unchecked((column_id - 1) as usize)) }
    }
}

pub(crate) fn init_record_store() {
    unsafe {
        RECORD_MAP = Some(SimpleU16Map::new());
        NAME_MAP = Some(HashMap::new());
    }
}

pub(crate) fn check_id_in_store(id: u16) -> bool {
    check_id(unsafe { ID_STORE }.as_slice(), id)
}
