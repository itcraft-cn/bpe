use crate::{
    aux::{fetch_f64, fetch_i64, fill_f64, fill_i64, SimpleU16Map},
    data::get_record,
    error::ParseSqlError,
    func::FnHolder,
    id::next_aggregate_id,
    sql::{
        base::{parse_options, ExprEntity, ParsedSql},
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
            // TODO: need to replace 1 with real id
            let map = unsafe { AGGREGATE_MAP.as_mut().unwrap() };
            map.insert(1, WrappedAggregate::new(aggregate, func_holder));
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
    for (idx, field) in wrapped._aggregate()._fields().iter().enumerate() {
        let rs = setup_init_val(&mut aggregate_data, idx, field);
        if rs.is_err() {
            log::warn!("hit error: {:?}", rs.err());
            return;
        }
    }
    for array in &data {
        compute(&mut aggregate_data, array, wrapped);
    }
    match &wrapped._fn_holder {
        FnHolder::Func(f) => f(vec![aggregate_data]),
        FnHolder::FfiFunc(f) => f.callback(vec![aggregate_data]),
        FnHolder::Lambda(f) => f(vec![aggregate_data]),
    }
}

fn setup_init_val(data: &mut [u8; 512], idx: usize, expr: &ExprEntity) -> Result<(), String> {
    let offset = idx * 8;
    match expr {
        ExprEntity::Function(_name, _expr_array) => {
            log::debug!("{} => {:?}", _name, _expr_array);
            match _name.as_str() {
                "_maxl" => {
                    fill_i64(&mut data[offset..offset + 8], i64::MIN);
                    Ok(())
                }
                "_minl" => {
                    fill_i64(&mut data[offset..offset + 8], i64::MAX);
                    Ok(())
                }
                "_suml" => {
                    fill_i64(&mut data[offset..offset + 8], 0);
                    Ok(())
                }
                "_countl" => {
                    fill_i64(&mut data[offset..offset + 8], 0);
                    Ok(())
                }
                "_maxd" => {
                    fill_f64(&mut data[offset..offset + 8], f64::MIN);
                    Ok(())
                }
                "_mind" => {
                    fill_f64(&mut data[offset..offset + 8], f64::MIN);
                    Ok(())
                }
                "_sumd" => {
                    fill_f64(&mut data[offset..offset + 8], 0_f64);
                    Ok(())
                }
                "_avgd" => {
                    fill_f64(&mut data[offset..offset + 8], 0_f64);
                    Ok(())
                }
                _ => {
                    log::warn!("unsupported function: {}", _name);
                    Err(String::from("unsupported function"))
                }
            }
        }
        _ => {
            log::warn!(
                "expr in top level just support aggregate func, this is not aggregate func:{:?}",
                expr
            );
            Err(String::from("not aggregate func"))
        }
    }
}

fn compute(data: &mut [u8; 512], _array: &[u8; 512], wrapped: &WrappedAggregate) {
    for (idx, expr) in wrapped._aggregate()._fields().iter().enumerate() {
        let offset = idx * 8;
        match expr {
            ExprEntity::Function(_name, _expr_array) => {
                log::debug!("{} => {:?}", _name, _expr_array);
                // TODO: 77_i64 and 77_f64 are the fake val, need to fetch real val
                match _name.as_str() {
                    "_maxl" => {
                        let max = fetch_i64(&data[offset..offset + 8]);
                        fill_i64(&mut data[offset..offset + 8], max.max(77_i64));
                    }
                    "_minl" => {
                        let min = fetch_i64(&data[offset..offset + 8]);
                        fill_i64(&mut data[offset..offset + 8], min.min(77_i64));
                    }
                    "_suml" => {
                        let sum = fetch_i64(&data[offset..offset + 8]);
                        fill_i64(&mut data[offset..offset + 8], sum + 77_i64);
                    }
                    "_countl" => {
                        let count = fetch_i64(&data[offset..offset + 8]);
                        fill_i64(&mut data[offset..offset + 8], count + 1);
                    }
                    "_maxd" => {
                        let max = fetch_f64(&data[offset..offset + 8]);
                        fill_f64(&mut data[offset..offset + 8], max.max(77_f64));
                    }
                    "_mind" => {
                        let min = fetch_f64(&data[offset..offset + 8]);
                        fill_f64(&mut data[offset..offset + 8], min.min(77_f64));
                    }
                    "_sumd" => {
                        let sum = fetch_f64(&data[offset..offset + 8]);
                        fill_f64(&mut data[offset..offset + 8], sum + 77_f64);
                    }
                    "_avgd" => {
                        let mut avg = fetch_f64(&data[offset..offset + 8]);
                        avg = avg * idx as f64 + 77_f64;
                        fill_f64(&mut data[offset..offset + 8], avg * idx as f64 + 77_f64);
                    }
                    _ => {
                        log::warn!("unsupported function: {}", _name);
                    }
                }
            }
            _ => {
                log::warn!("expr in top level just support aggregate func, this is not aggregate func:{:?}", expr);
            }
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
    let id = next_aggregate_id();
    Ok(Aggregate {
        _id: id,
        _fields: fields,
    })
}

pub(crate) struct Aggregate {
    _id: u16,
    _fields: Vec<ExprEntity>,
}
impl Aggregate {
    fn _fields(&self) -> &Vec<ExprEntity> {
        &self._fields
    }
}

pub(crate) struct WrappedAggregate {
    _aggregate: Aggregate,
    _fn_holder: FnHolder,
}
impl WrappedAggregate {
    fn new(_aggregate: Aggregate, _fn_holder: FnHolder) -> Self {
        WrappedAggregate {
            _aggregate,
            _fn_holder,
        }
    }
    fn _aggregate(&self) -> &Aggregate {
        &self._aggregate
    }
}
