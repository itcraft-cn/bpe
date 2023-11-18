use crate::{
    aux::{fetch_f64, fetch_i64, fill_f64, fill_i64, SimpleU16Map},
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

static mut AGGREGATE_MAP: Option<SimpleU16Map> = None;
static mut PARSE_OPTIONS: Option<ParseOptions> = None;

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
pub(crate) fn call_aggregate(wrapped: &WrappedAggregate, data: Vec<[u8; 512]>) {
    let mut aggregate_data = [0_u8; 512];
    for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        let rs = setup_init_val(&mut aggregate_data, idx, executor);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err());
            return;
        }
    }
    for (data_idx, sub_data) in data.iter().enumerate() {
        loop_compute(&mut aggregate_data, sub_data, data_idx, wrapped);
    }
    match &wrapped.fn_holder {
        FnHolder::Func(f) => f(vec![aggregate_data]),
        FnHolder::FfiFunc(f) => f.callback(vec![aggregate_data]),
        FnHolder::Lambda(f) => f(vec![aggregate_data]),
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
            _ => {
                Ok(())
            }
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
) {
    for (idx, executor) in wrapped.aggregate().executors().iter().enumerate() {
        compute(idx, executor, aggregate_data, sub_data, data_idx);
    }
}

#[inline]
fn compute(
    idx: usize,
    executor: &Executor,
    aggregate_data: &mut [u8; 512],
    sub_data: &[u8; 512],
    data_idx: usize,
) {
    let offset = idx * 8;
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
            let element = fetch_arg_val(sub_data, sub_executor);
            match (func, &element) {
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

fn fetch_arg_val(sub_data: &[u8; 512], executor: &Executor) -> Element {
    match executor {
        Executor::Fetch(record_id, field_id) => {
            if let Some(column) = Record::get_column(*record_id, *field_id) {
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
    let stream = Record::get_record(parsed_sql.records()[0]);
    if stream.is_none() {
        return Err(ParseSqlError::new(format!(
            "stream {} not found",
            parsed_sql.records()[0]
        )));
    }
    let rs = create_executor(&parsed_sql.records(), &fields);
    if let Ok(executors) = rs {
        let id = next_aggregate_id();
        Ok(Aggregate { id, executors })
    } else {
        Err(ParseSqlError::new(String::from(
            "fail to create executors for aggregate",
        )))
    }
}

pub(crate) struct Aggregate {
    id: u16,
    executors: Vec<Executor>,
}
impl Aggregate {
    fn id(&self) -> u16 {
        self.id
    }
    fn executors(&self) -> &Vec<Executor> {
        &self.executors
    }
}

pub(crate) struct WrappedAggregate {
    _aggregate: Aggregate,
    fn_holder: FnHolder,
}
impl WrappedAggregate {
    fn new(_aggregate: Aggregate, _fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            _aggregate,
            fn_holder: _fn_holder,
        }
    }
    fn aggregate(&self) -> &Aggregate {
        &self._aggregate
    }
}
