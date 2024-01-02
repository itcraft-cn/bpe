use crate::{
    aux::{check_id, fetch_ptr, fill_ptr, set_id, SimpleU16Map},
    consts::U8_DATA_MAX_SIZE,
    data::{ColumnType, Record},
    element::Element,
    error::ParseSqlError,
    exec::create_executor,
    func::{Executor, FnHolder, Func},
    id::next_aggregate_id,
    sql::{
        base::{parse_options, ParsedSql},
        select::parse_select,
    },
};
use sql_parse::ParseOptions;
use std::{
    alloc::{self, Layout},
    cell::RefCell,
    ptr,
};

static mut AGGREGATE_MAP: Option<SimpleU16Map> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;
thread_local! {
    static FIELD_REF :RefCell<[u8;8]>= RefCell::new([0_u8; 8]);
}

pub(crate) fn init_aggregate_store() {
    unsafe {
        PARSE_OPTIONS = Some(parse_options());
        AGGREGATE_MAP = Some(SimpleU16Map::new());
    }
}

pub(crate) fn define_aggregate(sql: &str, func_holder: FnHolder) -> Option<u16> {
    if let Some(parsed_sql) = parse_select(sql, unsafe { PARSE_OPTIONS.as_ref().unwrap() }) {
        let rs = gen_aggregate(&parsed_sql);
        if let Ok(aggregate) = rs {
            let map = unsafe { AGGREGATE_MAP.as_mut().unwrap() };
            map.insert(
                aggregate.id(),
                WrappedAggregate::new(aggregate, func_holder),
            );
            Some(1)
        } else {
            log::warn!("{:?}", rs.err());
            None
        }
    } else {
        None
    }
}

#[inline]
pub(crate) fn call_aggregate(wrapped: &WrappedAggregate, u8_ptr: *const u8, size: usize) {
    thread_local! {
        static DATA_REF :RefCell<u64>= RefCell::new(unsafe {alloc::alloc(Layout::from_size_align(U8_DATA_MAX_SIZE, 1).unwrap())} as u64);
    };
    let id = wrapped.aggregate().stream_id();
    if let Some(stream) = Record::get_record(id) {
        DATA_REF.with_borrow(|aggregate_data_ptr_val| {
            let aggregate_data_ptr = *aggregate_data_ptr_val as *mut u8;
            call_with_threadlocal(aggregate_data_ptr, wrapped, u8_ptr, stream, size);
        });
    } else {
        log::warn!("failed to find stream by id[{}]", id);
    }
}

#[inline]
fn call_with_threadlocal(
    aggregate_data_ptr: *mut u8,
    wrapped: &WrappedAggregate,
    u8_ptr: *const u8,
    stream: &Record,
    size: usize,
) {
    if init_data(aggregate_data_ptr, wrapped, stream) {
        compute_data(aggregate_data_ptr, u8_ptr, wrapped, stream, size);
        callback(wrapped, aggregate_data_ptr, 1);
    }
}

#[inline]
fn init_data(aggregate_data_ptr: *mut u8, wrapped: &WrappedAggregate, stream: &Record) -> bool {
    FIELD_REF.with_borrow_mut(|field_ref| {
        field_ref.fill(0_u8);
    });
    for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        let rs = setup_init_val(aggregate_data_ptr, idx, executor, stream);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err());
            return false;
        }
    }
    true
}

#[inline]
fn compute_data(
    aggregate_data_ptr: *mut u8,
    u8_ptr: *const u8,
    wrapped: &WrappedAggregate,
    stream: &Record,
    size: usize,
) {
    for data_idx in 0..size {
        loop_compute(
            aggregate_data_ptr,
            unsafe { u8_ptr.add(data_idx * U8_DATA_MAX_SIZE) },
            data_idx,
            wrapped,
            stream,
        );
    }
}

fn callback(wrapped: &WrappedAggregate, aggregate_data_ptr: *mut u8, size: usize) {
    match &wrapped.fn_holder {
        FnHolder::Func(f) => f(aggregate_data_ptr, size),
        FnHolder::FfiFunc(f) => f.callback(aggregate_data_ptr, size),
        FnHolder::Lambda(f) => f(aggregate_data_ptr, size),
    }
}

#[inline]
fn setup_init_val(
    aggregate_data_ptr: *mut u8,
    idx: usize,
    executor: &Executor,
    stream: &Record,
) -> Result<(), String> {
    let offset = stream.column((idx + 1) as u16).offset();
    match executor {
        Executor::Compute(func, _) => {
            init_for_some_func(func, aggregate_data_ptr, offset);
            Ok(())
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{:?}",
                executor
            );
            Err(String::from("not aggregate func"))
        }
    }
}

#[inline]
fn init_for_some_func(func: &Func, aggregate_data_ptr: *mut u8, offset: usize) {
    match func {
        Func::MaxL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MIN);
        }
        Func::MinL => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, i64::MAX);
        }
        Func::MaxD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MIN);
        }
        Func::MinD => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, f64::MAX);
        }
        _ => {}
    }
}

#[inline]
fn loop_compute(
    aggregate_data_ptr: *mut u8,
    sub_data: *const u8,
    data_idx: usize,
    wrapped: &WrappedAggregate,
    stream: &Record,
) {
    FIELD_REF.with_borrow_mut(|field_ref| {
        for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
            compute(
                idx,
                executor,
                aggregate_data_ptr,
                field_ref,
                sub_data,
                data_idx,
                stream,
            );
        }
    });
}

#[inline]
fn compute(
    idx: usize,
    executor: &Executor,
    aggregate_data_ptr: *mut u8,
    field_ref: &mut [u8; 8],
    sub_data: *const u8,
    data_idx: usize,
    stream: &Record,
) {
    let offset = stream.column((idx + 1) as u16).offset();
    match executor {
        Executor::Compute(func, executors) => {
            if executors.executor_size() != 1 {
                log::warn!(
                    "aggregate func[{:?}] just support one argument, here is {:?} executors",
                    func,
                    executors.executor_size()
                );
                return;
            }
            let sub_executor = executors.index_of(0);
            let element = fetch_arg_val(sub_data, sub_executor, stream);
            choose_func(
                func,
                element,
                field_ref,
                idx,
                aggregate_data_ptr,
                offset,
                data_idx,
            );
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{:?}",
                executor
            );
        }
    }
}

#[inline]
fn choose_func(
    func: &Func,
    element: Element,
    field_ref: &mut [u8; 8],
    idx: usize,

    aggregate_data_ptr: *mut u8,
    offset: usize,
    data_idx: usize,
) {
    match func {
        Func::Key => match &element {
            Element::Long(v) => {
                func_key_long(idx, aggregate_data_ptr, field_ref, offset, v);
            }
            Element::Double(v) => {
                func_key_double(idx, aggregate_data_ptr, field_ref, offset, v);
            }
            Element::Str(u64ptr, str_offset, len) => {
                func_key_str(
                    idx,
                    aggregate_data_ptr,
                    field_ref,
                    offset,
                    u64ptr,
                    str_offset,
                    len,
                );
            }
        },
        Func::MaxL => match &element {
            Element::Long(v) => {
                func_max_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::MinL => match &element {
            Element::Long(v) => {
                func_min_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::SumL => match &element {
            Element::Long(v) => {
                func_sum_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::Count => match &element {
            Element::Long(_) => {
                func_count_long(aggregate_data_ptr, offset);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::MaxD => match &element {
            Element::Long(v) => {
                func_maxd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                func_maxd_double(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::MinD => match &element {
            Element::Long(v) => {
                func_mind_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                func_mind_double(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::SumD => match &element {
            Element::Long(v) => {
                func_sumd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                func_sumd_double(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::Avg => match &element {
            Element::Long(v) => {
                func_avg_long(aggregate_data_ptr, offset, v, data_idx);
            }
            Element::Double(v) => {
                func_avg_double(aggregate_data_ptr, offset, v, data_idx);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        _ => {
            log::warn!("unsupported function: {:?}-{:?}", func, &element);
        }
    };
}

#[inline]
fn func_key_long(
    idx: usize,

    aggregate_data_ptr: *mut u8,
    field_ref: &mut [u8; 8],
    offset: usize,
    v: &i64,
) {
    if check_id(field_ref.as_slice(), idx as u16) {
        fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
        set_id(field_ref.as_mut_slice(), idx as u16);
    }
}
#[inline]
fn func_key_double(
    idx: usize,

    aggregate_data_ptr: *mut u8,
    field_ref: &mut [u8; 8],
    offset: usize,
    v: &f64,
) {
    if check_id(field_ref.as_slice(), idx as u16) {
        fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
        set_id(field_ref.as_mut_slice(), idx as u16);
    }
}
#[inline]
fn func_key_str(
    idx: usize,

    aggregate_data_ptr: *mut u8,
    field_ref: &mut [u8; 8],
    offset: usize,
    u64ptr: &u64,
    str_offset: &usize,
    len: &usize,
) {
    if check_id(field_ref.as_slice(), idx as u16) {
        let ptr_slice = (*u64ptr + *str_offset as u64) as *const u8;
        unsafe { ptr::copy_nonoverlapping(ptr_slice, aggregate_data_ptr.add(offset), *len) };
        set_id(field_ref.as_mut_slice(), idx as u16);
    }
}
#[inline]
fn func_max_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}
#[inline]
fn func_min_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}
#[inline]
fn func_sum_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}
#[inline]
fn func_count_long(aggregate_data_ptr: *mut u8, offset: usize) {
    let count: i64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, count + 1);
}
#[inline]
fn func_maxd_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        max.max(*v as f64),
    );
}
#[inline]
fn func_mind_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        min.min(*v as f64),
    );
}
#[inline]
fn func_sumd_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v as f64);
}
#[inline]
fn func_avg_long(aggregate_data_ptr: *mut u8, offset: usize, v: &i64, data_idx: usize) {
    let avg: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        (avg * (data_idx as f64) + (*v as f64)) / ((data_idx + 1) as f64),
    );
}
#[inline]
fn func_maxd_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let max: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, max.max(*v));
}
#[inline]
fn func_mind_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let min: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, min.min(*v));
}
#[inline]
fn func_sumd_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64) {
    let sum: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, sum + *v);
}
#[inline]
fn func_avg_double(aggregate_data_ptr: *mut u8, offset: usize, v: &f64, data_idx: usize) {
    let avg: f64 = fetch_ptr(unsafe { aggregate_data_ptr.add(offset) });
    fill_ptr(
        unsafe { aggregate_data_ptr.add(offset) },
        (avg * (data_idx as f64) + *v) / ((data_idx + 1) as f64),
    );
}

fn fetch_arg_val(sub_data: *const u8, executor: &Executor, stream: &Record) -> Element {
    match executor {
        Executor::Fetch(_record_id, field_id) => {
            let column = stream.column(*field_id);
            let column_type = column.data_type();
            let offset = column.offset();
            match column_type {
                ColumnType::Long => {
                    Element::Long(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
                ColumnType::Double => {
                    Element::Double(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
                ColumnType::Str(len) => Element::Str(sub_data as u64, offset, *len),
            }
        }
        _ => {
            log::warn!("just support fetch, here is {:?} executor", executor);
            Element::Long(0)
        }
    }
}

pub(crate) fn search_aggregate<'a>(id: u16) -> Option<&'a WrappedAggregate> {
    let map = unsafe { AGGREGATE_MAP.as_ref().unwrap() };
    map.get(id)
}

fn gen_aggregate(parsed_sql: &ParsedSql) -> Result<Aggregate, ParseSqlError> {
    let fields = parsed_sql.fields();
    if fields.is_empty() {
        return Err(ParseSqlError::new(String::from("no field in aggregate")));
    }
    if parsed_sql.records().len() != 1 {
        return Err(ParseSqlError::new(String::from(
            "only support one record in aggregate",
        )));
    }
    let opt_stream = Record::get_record(parsed_sql.records()[0]);
    if opt_stream.is_none() {
        return Err(ParseSqlError::new(format!(
            "stream {} not found",
            parsed_sql.records()[0]
        )));
    }
    let rs = create_executor(parsed_sql.records(), fields);
    if let Ok(executors) = rs {
        let id = next_aggregate_id();
        Ok(Aggregate {
            id,
            stream_id: opt_stream.unwrap().id(),
            executors,
        })
    } else {
        Err(ParseSqlError::new(String::from(
            "fail to create executors for aggregate",
        )))
    }
}

pub(crate) struct Aggregate {
    id: u16,
    stream_id: u16,
    executors: Vec<Executor>,
}
impl Aggregate {
    fn id(&self) -> u16 {
        self.id
    }
    fn stream_id(&self) -> u16 {
        self.stream_id
    }
    fn executors(&self) -> &Vec<Executor> {
        &self.executors
    }
}

pub(crate) struct WrappedAggregate {
    aggregate: Aggregate,
    fn_holder: FnHolder,
}
impl WrappedAggregate {
    fn new(aggregate: Aggregate, fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            aggregate,
            fn_holder,
        }
    }
    fn aggregate(&self) -> &Aggregate {
        &self.aggregate
    }
}
