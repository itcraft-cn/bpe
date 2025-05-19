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
    store::{get_vec_size, WrappedArray},
};
use globalvar::{def_global_ptr, get_global, get_global_mut};
use inkwell::execution_engine::JitFunction;
use std::alloc::{self, Layout};

static mut PTR_MAPPER_MAP: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static mut PTR_VAL_DATA_REF: u64 = 0;

pub(crate) fn init_mapper() {
    unsafe {
        PTR_MAPPER_MAP = def_global_ptr(SimpleU16Map::new());
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options());
        PTR_VAL_DATA_REF = alloc::alloc(Layout::from_size_align(get_vec_size(), 1).unwrap()) as u64;
    }
}

pub(crate) fn define_mapper(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS });
    if let Some(parsed_sql) = parse_select(sql, &options) {
        let rs = gen_mapper(parsed_sql);
        if let Ok(mapper) = rs {
            let id = mapper.id();
            let mapper_map = get_global_mut::<SimpleU16Map>(unsafe { PTR_MAPPER_MAP });
            let entry = mapper_map.entry(id);
            match entry {
                SimpleU16Entry::Exist(_) => {
                    log::warn!("id {} already exists, sql[{}] is skipped", id, sql);
                    None
                }
                SimpleU16Entry::NotExist(_) => {
                    mapper_map.insert(id, WrappedMapper::new(mapper, func_holder));
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

fn search_mapper(id: u16) -> Option<&'static WrappedMapper> {
    let mapper_map = get_global::<SimpleU16Map>(unsafe { PTR_MAPPER_MAP });
    mapper_map.get(id)
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
    let u8_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
    let size = loop_filter(array, mapper, id, record, u8_ptr);
    callback(fn_holder, u8_ptr, size);
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
