use crate::{
    agg_func,
    aux::{fetch_ptr, fill_ptr, SimpleU16Map},
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
use globalvar::{def_global_ptr, get_global, get_global_mut};
use std::alloc::{self, Layout};

static mut PTR_AGGREGATE_MAP: u64 = 0;
static mut PTR_PARSE_OPTIONS: u64 = 0;
static mut PTR_FIELD_REF: u64 = 0;
static mut PTR_VAL_DATA_REF: u64 = 0;

pub(crate) fn init_aggregate() {
    unsafe {
        PTR_AGGREGATE_MAP = def_global_ptr(SimpleU16Map::new());
        PTR_PARSE_OPTIONS = def_global_ptr(parse_options());
        PTR_FIELD_REF = def_global_ptr([0_u8; 8]);
        PTR_VAL_DATA_REF =
            alloc::alloc(Layout::from_size_align(U8_DATA_MAX_SIZE, 1).unwrap()) as u64;
    }
}

pub(crate) fn define_aggregate(sql: &str, func_holder: FnHolder) -> Option<u16> {
    let options = get_global(unsafe { PTR_PARSE_OPTIONS });
    if let Some(parsed_sql) = parse_select(sql, options) {
        let rs = gen_aggregate(&parsed_sql);
        if let Ok(aggregate) = rs {
            let map = get_global_mut::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP });
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
    let id = wrapped.aggregate().stream_id();
    if let Some(stream) = Record::get_record(id) {
        let aggregate_data_ptr = unsafe { PTR_VAL_DATA_REF } as *mut u8;
        call_with_aggregate_data(aggregate_data_ptr, wrapped, u8_ptr, stream, size);
    } else {
        log::warn!("failed to find stream by id[{id}]");
    }
}

#[inline]
fn call_with_aggregate_data(
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
    let field_ref = get_global_mut::<[u8; 8]>(unsafe { PTR_FIELD_REF });
    field_ref.fill(0_u8);
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
        Executor::ConstLong(_) => Ok(()),
        Executor::ConstDouble(_) => Ok(()),
        Executor::Compute(func, _) => {
            init_for_some_func(func, aggregate_data_ptr, offset);
            Ok(())
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
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
    let field_ref = get_global_mut::<[u8; 8]>(unsafe { PTR_FIELD_REF });
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
        Executor::ConstLong(v) => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
        }
        Executor::ConstDouble(v) => {
            fill_ptr(unsafe { aggregate_data_ptr.add(offset) }, *v);
        }
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
                "expr in top level just support aggregate func, this is not aggregate func:{executor:?}"
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
                agg_func::func_key_long(idx, aggregate_data_ptr, field_ref, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_key_double(idx, aggregate_data_ptr, field_ref, offset, v);
            }
        },
        Func::MaxL => match &element {
            Element::Long(v) => {
                agg_func::func_max_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::MinL => match &element {
            Element::Long(v) => {
                agg_func::func_min_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::SumL => match &element {
            Element::Long(v) => {
                agg_func::func_sum_long(aggregate_data_ptr, offset, v);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::Count => match &element {
            Element::Long(_) => {
                agg_func::func_count_long(aggregate_data_ptr, offset);
            }
            _ => {
                log::warn!("unsupported function: {:?}-{:?}", func, &element);
            }
        },
        Func::MaxD => match &element {
            Element::Long(v) => {
                agg_func::func_maxd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_maxd_double(aggregate_data_ptr, offset, v);
            }
        },
        Func::MinD => match &element {
            Element::Long(v) => {
                agg_func::func_mind_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_mind_double(aggregate_data_ptr, offset, v);
            }
        },
        Func::SumD => match &element {
            Element::Long(v) => {
                agg_func::func_sumd_long(aggregate_data_ptr, offset, v);
            }
            Element::Double(v) => {
                agg_func::func_sumd_double(aggregate_data_ptr, offset, v);
            }
        },
        Func::Avg => match &element {
            Element::Long(v) => {
                agg_func::func_avg_long(aggregate_data_ptr, offset, v, data_idx);
            }
            Element::Double(v) => {
                agg_func::func_avg_double(aggregate_data_ptr, offset, v, data_idx);
            }
        },
        _ => {
            log::warn!("unsupported function: {:?}-{:?}", func, &element);
        }
    };
}

fn fetch_arg_val(sub_data: *const u8, executor: &Executor, stream: &Record) -> Element {
    match executor {
        Executor::Fetch(_record_id, field_id) => {
            let column = stream.column(*field_id);
            let column_type = column.data_type();
            match column_type {
                ColumnType::Long => {
                    Element::Long(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
                ColumnType::Double => {
                    Element::Double(unsafe { fetch_ptr(sub_data.add(column.offset())) })
                }
            }
        }
        _ => {
            log::warn!("just support fetch, here is {executor:?} executor");
            Element::Long(0)
        }
    }
}

pub(crate) fn search_aggregate(id: u16) -> Option<&'static WrappedAggregate> {
    let map = get_global::<SimpleU16Map>(unsafe { PTR_AGGREGATE_MAP });
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
