use crate::{
    aux::{SimpleU16Map, bitmap_chk_id, bitmap_set_id},
    consts::{ID_MAX_SIZE, U8_DATA_MAX_SIZE},
    id::next_record_id,
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use hashbrown::HashMap;

static mut PTR_RECORD_MAP: u64 = 0;
static mut PTR_NAME_MAP: u64 = 0;
static mut PTR_ID_STORE: u64 = 0;

/// 8-byte aligned payload so raw column reads (i64/f64) from `U8Bytes::bytes()`
/// and JIT filter calls on the payload pointer are always aligned.
#[repr(align(8))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct AlignedBytes(pub(crate) [u8; U8_DATA_MAX_SIZE]);

#[derive(Clone, Copy, Debug)]
pub struct U8Bytes {
    id: u16,
    data_len: usize,
    bytes: AlignedBytes,
}
impl U8Bytes {
    pub fn new(id: u16, data_len: usize, bytes: [u8; U8_DATA_MAX_SIZE]) -> U8Bytes {
        U8Bytes {
            id,
            data_len,
            bytes: AlignedBytes(bytes),
        }
    }

    pub fn new_from_vec(id: u16, data_len: usize, vec: Vec<u8>) -> U8Bytes {
        let mut bytes = [0_u8; U8_DATA_MAX_SIZE];
        let slice = vec.as_slice();
        // clamp the effective length so data_len never exceeds the copied bytes
        let len = data_len.min(slice.len()).min(U8_DATA_MAX_SIZE);
        copy(len, &mut bytes, slice);
        U8Bytes {
            id,
            data_len: len,
            bytes: AlignedBytes(bytes),
        }
    }

    pub fn new_from_slice(id: u16, data_len: usize, slice: &[u8]) -> U8Bytes {
        let mut bytes = [0_u8; U8_DATA_MAX_SIZE];
        // clamp the effective length so data_len never exceeds the copied bytes
        let len = data_len.min(slice.len()).min(U8_DATA_MAX_SIZE);
        copy(len, &mut bytes, slice);
        U8Bytes {
            id,
            data_len: len,
            bytes: AlignedBytes(bytes),
        }
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn data_len(&self) -> usize {
        self.data_len
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.0.as_slice()
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.0.as_mut_slice()
    }
}

fn copy(len: usize, bytes: &mut [u8; U8_DATA_MAX_SIZE], slice: &[u8]) {
    bytes.as_mut_slice()[0..len].copy_from_slice(&slice[0..len]);
}

#[derive(Clone, Copy, Debug)]
pub enum ColumnType {
    Long,
    Double,
}
impl ColumnType {
    fn by_idx(type_idx: u16, _size: usize) -> ColumnType {
        match type_idx {
            0 => ColumnType::Long,
            1 => ColumnType::Double,
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
        // check name uniqueness FIRST, otherwise a duplicate name leaves a zombie record
        let key = String::from(name);
        let name_map = get_global_mut::<HashMap<String, u16>>(unsafe { PTR_NAME_MAP });
        if name_map.contains_key(&key) {
            log::warn!("Record name {name} already exists");
            None
        } else {
            let id = next_record_id();
            let record_map = get_global_mut::<SimpleU16Map>(unsafe { PTR_RECORD_MAP });
            record_map.insert(
                id,
                Record::new(name, id, record_type, Column::copy_from_columns(&columns)),
            );
            name_map.insert(key, id);
            let id_store = get_global_mut::<[u8; ID_MAX_SIZE]>(unsafe { PTR_ID_STORE });
            bitmap_set_id(id_store.as_mut_slice(), id);
            Some(id)
        }
    }

    pub(crate) fn get_record(id: u16) -> Option<&'static Record> {
        let record_map = get_global::<SimpleU16Map>(unsafe { PTR_RECORD_MAP });
        record_map.get(id)
    }

    pub(crate) fn fetch_record_id(name: &str) -> Option<u16> {
        let name_map = get_global::<HashMap<String, u16>>(unsafe { PTR_NAME_MAP });
        name_map.get(&String::from(name)).copied()
    }

    pub(crate) fn fetch_column_id(record_id: u16, column_name: &str) -> Option<&u16> {
        if let Some(record) = Record::get_record(record_id) {
            record.column_id(column_name)
        } else {
            None
        }
    }

    pub(crate) fn get_column(record_id: u16, column_id: u16) -> Option<&'static Column> {
        if let Some(record) = Record::get_record(record_id) {
            Some(record.column(column_id))
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

    pub(crate) fn _columns(&self) -> &Vec<Column> {
        &self.columns
    }

    pub(crate) fn column_id(&self, column_name: &str) -> Option<&u16> {
        self.columns_map.get(&String::from(column_name))
    }

    pub(crate) fn column(&self, column_id: u16) -> &Column {
        let idx = column_id as usize;
        if idx == 0 || idx > self.columns.len() {
            // defensive guard: never index out of bounds (release would abort otherwise)
            log::warn!(
                "column id [{column_id}] is out of range, column count: {}",
                self.columns.len()
            );
            &self.columns[0]
        } else {
            unsafe { self.columns.get_unchecked(idx - 1) }
        }
    }
}

/// Checks if a record ID is in the store (i.e., has been defined).
/// Returns true if the ID is not in the store (meaning it's undefined), false otherwise.
pub(crate) fn check_id_in_store(id: u16) -> bool {
    let id_store = get_global_mut::<[u8; ID_MAX_SIZE]>(unsafe { PTR_ID_STORE });  // Get ID store
    bitmap_chk_id(id_store.as_slice(), id)  // Check if ID is set in the bitmap
}

/// Initializes the data system by setting up global pointers for:
/// - Record map to store record definitions
/// - Name map to map record names to IDs
/// - ID store to track which IDs are in use
pub(crate) fn init_data() {
    unsafe {
        PTR_RECORD_MAP = def_global_ptr(SimpleU16Map::new());              // Initialize record map
        PTR_NAME_MAP = def_global_ptr::<HashMap<String, u16>>(HashMap::new());  // Initialize name map
        PTR_ID_STORE = def_global_ptr([0; ID_MAX_SIZE]);                         // Initialize ID store
    }
}