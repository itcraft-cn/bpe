use crate::{
    aux::{SimpleU16Entry, SimpleU16Map},
    consts::U8_DATA_MAX_SIZE,
    data::Record,
    error::ParseSqlError,
    exec::create_executor,
    func::{Executors, FnHolder},
    id::next_mapper_id,
    sql::{
        base::{parse_options, FilterFunc, ParsedSql},
        select::parse_select,
    },
    store::{WrappedArray, VEC_SIZE},
};
use inkwell::execution_engine::JitFunction;
use sql_parse::ParseOptions;
use std::{
    alloc::{self, Layout},
    cell::RefCell,
};

static mut MAPPER_MAP: Option<SimpleU16Map> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

pub(crate) fn init_mapper_store() {
    unsafe {
        PARSE_OPTIONS = Some(parse_options());
        MAPPER_MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn define_mapper(sql: &str, func_holder: FnHolder) -> Option<u16> {
    if let Some(parsed_sql) = parse_select(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() }) {
        let rs = gen_mapper(parsed_sql);
        if let Ok(mapper) = rs {
            let id = mapper.id();
            let map = unsafe { MAPPER_MAP.as_mut().unwrap() };
            let entry = map.entry(id);
            match entry {
                SimpleU16Entry::Exist(_) => {
                    log::warn!("id {} already exists, sql[{}] is skipped", id, sql);
                    None
                }
                SimpleU16Entry::NotExist(_) => {
                    map.insert(id, WrappedMapper::new(mapper, func_holder));
                    Some(id)
                }
            }
        } else {
            log::warn!(
                "fail to create mapper from sql[{}], hit unexpected error: {:?}",
                sql,
                rs.err().unwrap()
            );
            None
        }
    } else {
        log::warn!("not supported sql statement: [{}]", sql);
        None
    }
}

#[inline]
pub(crate) fn call_mapper(array: &WrappedArray, id: u16) {
    let opt_mappers = search_mapper(id);
    let opt_record = Record::get_record(id);
    if opt_mappers.is_none() || opt_record.is_none() {
        return;
    }
    let wrapped_mapper = opt_mappers.unwrap();
    let record = opt_record.unwrap();
    invoke(
        id,
        array,
        &wrapped_mapper.mapper,
        record,
        &wrapped_mapper.fn_holder,
    );
}

fn search_mapper<'a>(id: u16) -> Option<&'a WrappedMapper> {
    let map = unsafe { MAPPER_MAP.as_ref().unwrap() };
    map.get(id)
}

fn gen_mapper(parsed_sql: ParsedSql) -> Result<Mapper, ParseSqlError> {
    let record_id_array = &parsed_sql.records();
    if record_id_array.len() != 1 {
        return Err(ParseSqlError::new(format!(
            "only support one record, but {} records",
            record_id_array.len()
        )));
    }
    let id = next_mapper_id();
    let rs_executors = create_executor(record_id_array, parsed_sql.fields());
    if rs_executors.is_err() {
        return Err(ParseSqlError::new(format!(
            "failed to parse executors: {}",
            rs_executors.err().unwrap()
        )));
    }
    let vec_executors = rs_executors.unwrap();
    Ok(Mapper::new(
        id,
        parsed_sql,
        Executors::new(vec_executors.as_slice()),
    ))
}

#[inline]
fn invoke(
    id: u16,
    array: &WrappedArray,
    mapper: &'static Mapper,
    record: &Record,
    fn_holder: &FnHolder,
) {
    thread_local! {
        static DATA_REF :RefCell<u64> = RefCell::new(unsafe {alloc::alloc(Layout::from_size_align(VEC_SIZE, 1).unwrap())} as u64);
    };
    DATA_REF.with_borrow(|u8_ptr_val| {
        let u8_ptr = *u8_ptr_val as *mut u8;
        let size = loop_filter(array, mapper, id, record, u8_ptr);
        callback(fn_holder, u8_ptr, size);
    })
}

fn loop_filter(
    array: &WrappedArray,
    mapper: &Mapper,
    id: u16,
    record: &Record,
    u8_ptr: *mut u8,
) -> usize {
    let mut idx = 0;
    let walker = array.walker() + array.size();
    let mask = array.mask();
    let max_idx = array.records() - 1;
    let u64ptr = array.u64ptr();
    let mut n = 0;
    let mut offset = 0;
    loop {
        let position = (walker - U8_DATA_MAX_SIZE - idx * U8_DATA_MAX_SIZE) & mask;
        let sub_data_ptr = array.sub_data(position);
        if unsafe { mapper.filter().call(u64ptr) } {
            //log::info!("position: {}", position);
            let adjusted = unsafe { u8_ptr.add(offset) };
            mapper.fetch(id, u64ptr, record, position, sub_data_ptr, adjusted);
            n += 1;
            offset += n * U8_DATA_MAX_SIZE;
            if n == mapper.limit() {
                //log::info!("quit, hit limit: {}", n);
                break;
            }
        }
        if idx == max_idx {
            //log::info!("quit, max idx: {}", idx);
            break;
        } else {
            idx += 1;
        }
    }
    n
}

fn callback(fn_holder: &FnHolder, u8_ptr: *mut u8, size: usize) {
    match fn_holder {
        FnHolder::Func(f) => f(u8_ptr, size),
        FnHolder::FfiFunc(ffi) => ffi.callback(u8_ptr, size),
        FnHolder::Lambda(f) => f(u8_ptr, size),
    }
}

#[derive(Debug)]
pub(crate) struct Mapper {
    id: u16,
    parsed_sql: ParsedSql,
    executors: Executors,
}
impl Mapper {
    fn new(id: u16, parsed_sql: ParsedSql, executors: Executors) -> Mapper {
        Self {
            id,
            parsed_sql,
            executors,
        }
    }

    pub(crate) fn id(&self) -> u16 {
        self.id
    }

    fn fetch(
        &self,
        id: u16,
        u64ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
        target: *const u8,
    ) {
        let mut val;
        let mut offset = 0_usize;
        let executors = &self.executors;
        let len = executors.executor_size();
        let mut i = 0;
        loop {
            let executor = executors.index_of(i);
            val = executor.fetch(id, u64ptr, record, position, sub_data_ptr);
            let val_len = val.len();
            val.copy_to_target(target as *mut u8, offset);
            offset += val_len;
            i += 1;
            if i >= len {
                break;
            }
        }
    }

    fn filter(&self) -> &JitFunction<FilterFunc> {
        self.parsed_sql.filter()
    }

    fn limit(&self) -> usize {
        self.parsed_sql.limit()
    }
}

pub(crate) struct WrappedMapper {
    mapper: Mapper,
    fn_holder: FnHolder,
}
impl WrappedMapper {
    fn new(mapper: Mapper, fn_holder: FnHolder) -> Self {
        WrappedMapper { mapper, fn_holder }
    }
}
