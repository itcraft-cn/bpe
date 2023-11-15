use crate::{
    aux::{fetch_f64, fetch_i64, fill_f64, fill_i64, SimpleU16Map},
    data::get_record,
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

static mut AGGREGATE_MAP: Option<SimpleU16Map<WrappedAggregate>> = None;
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
    let mut data_idx = 0_usize;
    for sub_data in &data {
        loop_compute(&mut aggregate_data, sub_data, data_idx, wrapped);
        data_idx += 1;
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
            Func::SumL => {
                fill_i64(&mut aggregate_data[offset..offset + 8], 0);
                Ok(())
            }
            Func::Count => {
                fill_i64(&mut aggregate_data[offset..offset + 8], 0);
                Ok(())
            }
            Func::MaxD => {
                fill_f64(&mut aggregate_data[offset..offset + 8], f64::MIN);
                Ok(())
            }
            Func::MinD => {
                fill_f64(&mut aggregate_data[offset..offset + 8], f64::MIN);
                Ok(())
            }
            Func::SumD => {
                fill_f64(&mut aggregate_data[offset..offset + 8], 0_f64);
                Ok(())
            }
            Func::Avg => {
                fill_f64(&mut aggregate_data[offset..offset + 8], 0_f64);
                Ok(())
            }
            _ => {
                log::warn!("unsupported function: {:?}", func);
                Err(String::from("unsupported function"))
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
    data: &mut [u8; 512],
    _sub_data: &[u8; 512],
    data_idx: usize,
) {
    let offset = idx * 8;
    match executor {
        Executor::Compute(func, _executors) => {
            // TODO: 77_i64 and 77_f64 are the fake val, need to fetch real val
            match func {
                Func::MaxL => {
                    let max = fetch_i64(&data[offset..offset + 8]);
                    fill_i64(&mut data[offset..offset + 8], max.max(77_i64));
                }
                Func::MinL => {
                    let min = fetch_i64(&data[offset..offset + 8]);
                    fill_i64(&mut data[offset..offset + 8], min.min(77_i64));
                }
                Func::SumL => {
                    let sum = fetch_i64(&data[offset..offset + 8]);
                    fill_i64(&mut data[offset..offset + 8], sum + 77_i64);
                }
                Func::Count => {
                    let count = fetch_i64(&data[offset..offset + 8]);
                    fill_i64(&mut data[offset..offset + 8], count + 1);
                }
                Func::MaxD => {
                    let max = fetch_f64(&data[offset..offset + 8]);
                    fill_f64(&mut data[offset..offset + 8], max.max(77_f64));
                }
                Func::MinD => {
                    let min = fetch_f64(&data[offset..offset + 8]);
                    fill_f64(&mut data[offset..offset + 8], min.min(77_f64));
                }
                Func::SumD => {
                    let sum = fetch_f64(&data[offset..offset + 8]);
                    fill_f64(&mut data[offset..offset + 8], sum + 77_f64);
                }
                Func::Avg => {
                    let mut avg = fetch_f64(&data[offset..offset + 8]);
                    avg = avg * data_idx as f64 + 77_f64;
                    fill_f64(
                        &mut data[offset..offset + 8],
                        (avg * data_idx as f64 + 77_f64) / ((data_idx + 1) as f64),
                    );
                }
                _ => {
                    log::warn!("unsupported function: {:?}", func);
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
    let stream = get_record(parsed_sql.records()[0]);
    if stream.is_none() {
        return Err(ParseSqlError::new(format!(
            "stream {} not found",
            parsed_sql.records()[0]
        )));
    }
    let rs = create_executor(&parsed_sql.records(), &fields);
    if let Ok(executors) = rs {
        let id = next_aggregate_id();
        Ok(Aggregate {
            _id: id,
            _executors: executors,
        })
    } else {
        Err(ParseSqlError::new(String::from(
            "fail to create executors for aggregate",
        )))
    }
}

pub(crate) struct Aggregate {
    _id: u16,
    _executors: Vec<Executor>,
}
impl Aggregate {
    fn id(&self) -> u16 {
        self._id
    }
    fn executors(&self) -> &Vec<Executor> {
        &self._executors
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
