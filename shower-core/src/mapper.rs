use crate::{
    aux::{SimpleU16Entry, SimpleU16Map},
    consts::U8_DATA_MAX_SIZE,
    data::Record,
    element::Element,
    error::ParseSqlError,
    exec::create_executor,
    func::{eq, fetch_val, gt, gt_eq, lt, lt_eq, neq, Executors, FnHolder},
    id::next_mapper_id,
    sql::{
        base::{parse_options, ExprEntity, OpType, ParsedSql, ValType},
        select::parse_select,
    },
    store::{WrappedArray, VEC_SIZE},
};
use sql_parse::ParseOptions;
use std::{
    alloc::{self, Layout},
    cell::RefCell,
    ptr,
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
        let rs = gen_mapper(&parsed_sql);
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

fn gen_mapper(parsed_sql: &ParsedSql) -> Result<Mapper, ParseSqlError> {
    let record_id_array = &parsed_sql.records();
    if record_id_array.len() != 1 {
        return Err(ParseSqlError::new(format!(
            "only support one record, but {} records",
            record_id_array.len()
        )));
    }
    let id = next_mapper_id();
    let rs_filter = create_filter(parsed_sql.filters());
    if rs_filter.is_err() {
        return Err(ParseSqlError::new(format!(
            "failed to parse filter: {}",
            rs_filter.err().unwrap()
        )));
    }
    let rs_executors = create_executor(record_id_array, parsed_sql.fields());
    if rs_executors.is_err() {
        return Err(ParseSqlError::new(format!(
            "failed to parse executors: {}",
            rs_executors.err().unwrap()
        )));
    }
    let vec_executors = rs_executors.unwrap();
    Ok(Mapper {
        id,
        filter: rs_filter.unwrap(),
        limit: parsed_sql.limit(),
        executors: Executors::new(vec_executors.as_slice()),
    })
}

fn create_filter(entities: &[ExprEntity]) -> Result<Filter, ParseSqlError> {
    if entities.is_empty() {
        return Ok(Filter::Empty);
    }
    let mut filters: Vec<Filter> = entities
        .iter()
        .map(|e| Filter::Original(e.clone()))
        .collect();
    filters.reverse();
    let mut tmp: Vec<Filter> = vec![];
    loop {
        if filters.is_empty() {
            break;
        }
        let filter = filters.remove(0);
        if match &filter {
            Filter::Original(expr) => matches!(expr, ExprEntity::Op(_)),
            _ => false,
        } {
            let m2 = tmp.pop().unwrap();
            let m1 = tmp.pop().unwrap();
            tmp.push(Filter::Mixed(vec![m1, m2, filter]));
        } else {
            tmp.push(filter);
        }
    }
    if tmp.len() == 1 {
        Ok(tmp.first().unwrap().clone())
    } else {
        Err(ParseSqlError::new(
            "the last element is not found.".to_owned(),
        ))
    }
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
        if mapper
            .filter
            .is_match(id, u64ptr, record, position, sub_data_ptr)
        {
            let adjusted = unsafe { u8_ptr.add(offset) };
            mapper.fetch(id, u64ptr, record, position, sub_data_ptr, adjusted);
            n += 1;
            offset += n * U8_DATA_MAX_SIZE;
            if n == mapper.limit {
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

#[inline]
fn op_or(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    op_logical(sub_data_ptr, id, u64ptr, record, position, v1, v2, or)
}
#[inline]
fn op_and(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    op_logical(sub_data_ptr, id, u64ptr, record, position, v1, v2, and)
}
#[inline]
fn op_logical<F>(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
    f: F,
) -> bool
where
    F: Fn(*const u8, u16, u64, &Record, usize, &Filter, &Filter) -> bool,
{
    match (v1, v2) {
        (Filter::Mixed(_), Filter::Mixed(_)) => {
            f(sub_data_ptr, id, u64ptr, record, position, v1, v2)
        }
        _ => false,
    }
}

#[inline]
fn or(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    v1.is_match(id, u64ptr, record, position, sub_data_ptr)
        || v2.is_match(id, u64ptr, record, position, sub_data_ptr)
}

#[inline]
fn and(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    v1.is_match(id, u64ptr, record, position, sub_data_ptr)
        && v2.is_match(id, u64ptr, record, position, sub_data_ptr)
}

#[inline]
fn op_eq(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, eq)
}
#[inline]
fn op_gt_eq(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, gt_eq)
}
#[inline]
fn op_gt(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, gt)
}
#[inline]
fn op_lt_eq(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, lt_eq)
}
#[inline]
fn op_lt(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, lt)
}
#[inline]
fn op_neq(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
) -> bool {
    compare_slice_val(sub_data_ptr, id, u64ptr, record, position, v1, v2, neq)
}

#[inline]
fn compare_slice_val<F>(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    v1: &Filter,
    v2: &Filter,
    f: F,
) -> bool
where
    F: Fn(Element, Element) -> bool,
{
    match (v1, v2) {
        (Filter::Original(expr1), Filter::Original(expr2)) => match (expr1, expr2) {
            (ExprEntity::Field(idx), ExprEntity::Val(v_type)) => {
                compare_with_op(sub_data_ptr, id, u64ptr, record, position, *idx, v_type, f)
            }
            (ExprEntity::FieldWithTab(_, field_idx), ExprEntity::Val(v_type)) => compare_with_op(
                sub_data_ptr,
                id,
                u64ptr,
                record,
                position,
                *field_idx,
                v_type,
                f,
            ),
            (ExprEntity::Val(v_type), ExprEntity::Field(idx)) => {
                compare_with_op(sub_data_ptr, id, u64ptr, record, position, *idx, v_type, f)
            }
            (ExprEntity::Val(v_type), ExprEntity::FieldWithTab(_, field_idx)) => compare_with_op(
                sub_data_ptr,
                id,
                u64ptr,
                record,
                position,
                *field_idx,
                v_type,
                f,
            ),
            _ => false,
        },
        _ => false,
    }
}

#[inline]
fn compare_with_op<F>(
    sub_data_ptr: *const u8,
    id: u16,
    u64ptr: u64,
    record: &Record,
    position: usize,
    idx: u16,
    v_type: &ValType,
    f: F,
) -> bool
where
    F: Fn(Element, Element) -> bool,
{
    let opt_expacted = fetch_expacted(v_type);
    if let Some(expacted) = opt_expacted {
        let val = fetch_val(sub_data_ptr, id, u64ptr, record, position, idx);
        compare_val(expacted, val, f)
    } else {
        false
    }
}

#[inline]
fn fetch_expacted(v_type: &ValType) -> Option<Element> {
    match v_type {
        ValType::Int(val) => Some(Element::Long(*val)),
        ValType::Float(val) => Some(Element::Double(*val)),
        _ => {
            log::warn!(
                "not a valid data type: {:?}, not supported, skipping",
                v_type
            );
            None
        }
    }
}

#[inline]
fn compare_val<F>(expacted: Element, val: Element, f: F) -> bool
where
    F: Fn(Element, Element) -> bool,
{
    //log::info!("comparing {:?} and {:?}", &expacted, &val);
    f(expacted, val)
}

#[derive(Debug)]
pub(crate) struct Mapper {
    id: u16,
    filter: Filter,
    limit: usize,
    executors: Executors,
}
impl Mapper {
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
        for i in 0..len {
            let executor = executors.index_of(i);
            val = executor.fetch(id, u64ptr, record, position, sub_data_ptr);
            let val_len = val.len();
            val.copy_to_target(target as *mut u8, offset);
            offset += val_len;
        }
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

#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Empty,
    Original(ExprEntity),
    Mixed(Vec<Filter>),
}
impl Filter {
    fn is_match(
        &self,
        id: u16,
        u64ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
    ) -> bool {
        match self {
            Filter::Empty => true,
            Filter::Original(_) => false,
            Filter::Mixed(filters) => {
                self.filter_slice(id, u64ptr, record, position, filters, sub_data_ptr)
            }
        }
    }

    fn filter_slice(
        &self,
        id: u16,
        u64ptr: u64,
        record: &Record,
        position: usize,
        filters: &[Filter],
        sub_data_ptr: *const u8,
    ) -> bool {
        let v1 = &filters[0];
        let v2 = &filters[1];
        let op = &filters[2];
        match op {
            Filter::Empty => true,
            Filter::Original(expr) => {
                self.filter_slice_by_expr(id, u64ptr, record, position, expr, v1, v2, sub_data_ptr)
            }
            Filter::Mixed(_) => false,
        }
    }

    fn filter_slice_by_expr(
        &self,
        id: u16,
        u64ptr: u64,
        record: &Record,
        position: usize,
        expr: &ExprEntity,
        v1: &Filter,
        v2: &Filter,
        sub_data_ptr: *const u8,
    ) -> bool {
        match expr {
            ExprEntity::Op(op_type) => match op_type {
                OpType::Or => op_or(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::And => op_and(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::Eq => op_eq(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::GtEq => op_gt_eq(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::Gt => op_gt(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::LtEq => op_lt_eq(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::Lt => op_lt(sub_data_ptr, id, u64ptr, record, position, v1, v2),
                OpType::Neq => op_neq(sub_data_ptr, id, u64ptr, record, position, v1, v2),
            },
            _ => false,
        }
    }
}
