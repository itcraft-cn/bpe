use crate::{
    aux::{check_id, fetch_f64, fetch_i64, fill_f64, fill_i64, set_id, SimpleU16Map},
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
use std::cell::RefCell;

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
pub(crate) fn call_aggregate(wrapped: &WrappedAggregate, data: &Vec<[u8; 512]>) {
    thread_local! {
        static DATA_REF :RefCell<[u8;512]>= RefCell::new([0_u8; 512]);
    };
    let id = wrapped.aggregate().stream_id();
    if let Some(stream) = Record::get_record(id) {
        DATA_REF.with_borrow_mut(|aggregate_data| {
            call_with_threadlocal(aggregate_data, wrapped, data, stream);
        });
    } else {
        log::warn!("failed to find stream by id[{}]", id);
    }
}

fn call_with_threadlocal(
    aggregate_data: &mut [u8; 512],
    wrapped: &WrappedAggregate,
    data: &Vec<[u8; 512]>,
    stream: &Record,
) {
    if init_data(aggregate_data, wrapped) {
        compute_data(aggregate_data, data, wrapped, stream);
        callback(wrapped, aggregate_data);
    }
}

fn init_data(aggregate_data: &mut [u8; 512], wrapped: &WrappedAggregate) -> bool {
    FIELD_REF.with_borrow_mut(|field_ref| {
        field_ref.fill(0_u8);
    });
    for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        let rs = setup_init_val(aggregate_data, idx, executor);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err());
            return false;
        }
    }
    true
}

fn compute_data(
    aggregate_data: &mut [u8; 512],
    data: &Vec<[u8; 512]>,
    wrapped: &WrappedAggregate,
    stream: &Record,
) {
    for (data_idx, sub_data) in data.iter().enumerate() {
        loop_compute(aggregate_data, sub_data, data_idx, wrapped, stream);
    }
}

fn callback(wrapped: &WrappedAggregate, aggregate_data: &mut [u8; 512]) {
    match &wrapped.fn_holder {
        FnHolder::Func(f) => f(&vec![*aggregate_data]),
        FnHolder::FfiFunc(f) => f.callback(&vec![*aggregate_data]),
        FnHolder::Lambda(f) => f(&vec![*aggregate_data]),
    }
}

#[inline]
fn setup_init_val(
    aggregate_data: &mut [u8; 512],
    idx: usize,
    executor: &Executor,
) -> Result<(), String> {
    let offset = idx * 8;
    match executor {
        Executor::Compute(func, _) => match func {
            Func::MaxL => {
                fill_i64(&mut aggregate_data[offset..offset + 8], i64::MIN);
                Ok(())
            }
            Func::MinL => {
                fill_i64(&mut aggregate_data[offset..offset + 8], i64::MAX);
                Ok(())
            }
            Func::MaxD => {
                fill_f64(&mut aggregate_data[offset..offset + 8], f64::MIN);
                Ok(())
            }
            Func::MinD => {
                fill_f64(&mut aggregate_data[offset..offset + 8], f64::MAX);
                Ok(())
            }
            _ => Ok(()),
        },
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
fn loop_compute(
    aggregate_data: &mut [u8; 512],
    sub_data: &[u8; 512],
    data_idx: usize,
    wrapped: &WrappedAggregate,
    stream: &Record,
) {
    FIELD_REF.with_borrow_mut(|field_ref| {
        for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
            compute(
                idx,
                executor,
                aggregate_data,
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
    aggregate_data: &mut [u8; 512],
    field_ref: &mut [u8; 8],
    sub_data: &[u8; 512],
    data_idx: usize,
    stream: &Record,
) {
    let offset = stream.column((idx + 1) as u16).unwrap().offset();
    match executor {
        Executor::Compute(func, executors) => {
            if executors.len() != 1 {
                log::warn!(
                    "aggregate func[{:?}] just support one argument, here is {:?} executors",
                    func,
                    executors.len()
                );
                return;
            }
            let sub_executor = executors.first().unwrap();
            let element = fetch_arg_val(sub_data, sub_executor, stream);
            match (func, &element) {
                (Func::Key, Element::Long(v)) => {
                    if check_id(field_ref.as_slice(), idx as u16) {
                        fill_i64(&mut aggregate_data[offset..offset + 8], *v);
                        set_id(field_ref.as_mut_slice(), idx as u16);
                    }
                }
                (Func::Key, Element::Double(v)) => {
                    if check_id(field_ref.as_slice(), idx as u16) {
                        fill_f64(&mut aggregate_data[offset..offset + 8], *v);
                        set_id(field_ref.as_mut_slice(), idx as u16);
                    }
                }
                (Func::Key, Element::Str(_u64ptr, _offset, _len)) => {}
                (Func::MaxL, Element::Long(v)) => {
                    let max = fetch_i64(&aggregate_data[offset..offset + 8]);
                    fill_i64(&mut aggregate_data[offset..offset + 8], max.max(*v));
                }
                (Func::MinL, Element::Long(v)) => {
                    let min = fetch_i64(&aggregate_data[offset..offset + 8]);
                    fill_i64(&mut aggregate_data[offset..offset + 8], min.min(*v));
                }
                (Func::SumL, Element::Long(v)) => {
                    let sum = fetch_i64(&aggregate_data[offset..offset + 8]);
                    fill_i64(&mut aggregate_data[offset..offset + 8], sum + *v);
                }
                (Func::Count, Element::Long(_)) => {
                    let count = fetch_i64(&aggregate_data[offset..offset + 8]);
                    fill_i64(&mut aggregate_data[offset..offset + 8], count + 1);
                }
                (Func::MaxD, Element::Long(v)) => {
                    let max = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], max.max(*v as f64));
                }
                (Func::MinD, Element::Long(v)) => {
                    let min = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], min.min(*v as f64));
                }
                (Func::SumD, Element::Long(v)) => {
                    let sum = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], sum + *v as f64);
                }
                (Func::Avg, Element::Long(v)) => {
                    let avg = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(
                        &mut aggregate_data[offset..offset + 8],
                        (avg * (data_idx as f64) + (*v as f64)) / ((data_idx + 1) as f64),
                    );
                }
                (Func::MaxD, Element::Double(v)) => {
                    let max = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], max.max(*v));
                }
                (Func::MinD, Element::Double(v)) => {
                    let min = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], min.min(*v));
                }
                (Func::SumD, Element::Double(v)) => {
                    let sum = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(&mut aggregate_data[offset..offset + 8], sum + *v);
                }
                (Func::Avg, Element::Double(v)) => {
                    let avg = fetch_f64(&aggregate_data[offset..offset + 8]);
                    fill_f64(
                        &mut aggregate_data[offset..offset + 8],
                        (avg * (data_idx as f64) + *v) / ((data_idx + 1) as f64),
                    );
                }
                _ => {
                    log::warn!("unsupported function: {:?}-{:?}", func, &element);
                }
            }
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{:?}",
                executor
            );
        }
    }
}

fn fetch_arg_val(sub_data: &[u8; 512], executor: &Executor, stream: &Record) -> Element {
    match executor {
        Executor::Fetch(record_id, field_id) => {
            if let Some(column) = stream.column(*field_id) {
                let column_type = column.data_type();
                let offset = column.offset();
                let slice = sub_data.as_slice();
                match column_type {
                    ColumnType::Long => Element::Long(fetch_i64(&slice[offset..offset + 8])),
                    ColumnType::Double => Element::Double(fetch_f64(&slice[offset..offset + 8])),
                    ColumnType::Str(_) => Element::Long(0),
                }
            } else {
                log::warn!(
                    "fail to fetch record and column: [{}-{}]",
                    *record_id,
                    *field_id
                );
                Element::Long(0)
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
    if !parsed_sql.filters().is_empty() {
        return Err(ParseSqlError::new(String::from(
            "filter in aggregate is not supported",
        )));
    }
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
    let rs = create_executor(&parsed_sql.records(), &fields);
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
