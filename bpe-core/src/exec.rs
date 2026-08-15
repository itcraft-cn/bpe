use crate::{
    aux::fetch_ptr,
    calc_func,
    data::{ColumnType, Record},
    element::Element,
    error::ParseSqlError,
    func_enum::SupportFunc,
    sql::base::{ExprEntity, ValType},
};
use std::{
    alloc::{self, Layout},
    ptr,
    str::FromStr,
};

#[derive(Debug, Clone)]
pub(crate) struct Executors {
    raw_ptr: *const Executor,
    size: usize,
}
impl Executors {
    pub(crate) fn new(executors: &[Executor]) -> Self {
        let align_of_executor = align_of::<Executor>();
        let len = executors.len();
        let layout = Layout::from_size_align(size_of_val(executors), align_of_executor).unwrap();
        let raw_ptr = unsafe { alloc::alloc(layout) };
        let executor_ptr = raw_ptr.cast::<Executor>();
        for (i, item) in executors.iter().enumerate() {
            unsafe {
                let target_ptr = executor_ptr.add(i);
                ptr::write(target_ptr, item.clone());
            }
        }
        Self {
            raw_ptr: executor_ptr,
            size: len,
        }
    }

    pub(crate) fn executor_size(&self) -> i32 {
        self.size as i32
    }

    pub(crate) fn index_of(&self, idx: i32) -> &Executor {
        unsafe { &*self.raw_ptr.add(idx as usize) }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Executor {
    ConstLong(i64),
    ConstDouble(f64),
    Fetch(u16, u16),
    Compute(SupportFunc, Executors),
}
impl Executor {
    pub(crate) fn fetch(
        &self,
        id: u16,
        v_ptr: u64,
        record: &Record,
        position: usize,
        sub_data_ptr: *const u8,
    ) -> Element {
        match self {
            Executor::ConstLong(v) => Element::Long(*v),
            Executor::ConstDouble(v) => Element::Double(*v),
            Executor::Fetch(_, field_id) => {
                fetch_val(sub_data_ptr, id, v_ptr, record, position, *field_id)
            }
            Executor::Compute(f, executors) => {
                compute_func(sub_data_ptr, id, v_ptr, record, position, f, executors)
            }
        }
    }
}

fn fetch_val(
    sub_data_ptr: *const u8,
    _id: u16,
    _v_ptr: u64,
    record: &Record,
    _position: usize,
    idx: u16,
) -> Element {
    let column = record.column(idx);
    match column.data_type() {
        ColumnType::Long => Element::Long(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) })),
        ColumnType::Double => {
            Element::Double(fetch_ptr(unsafe { sub_data_ptr.add(column.offset()) }))
        }
    }
}

fn compute_func(
    sub_data_ptr: *const u8,
    id: u16,
    v_ptr: u64,
    record: &Record,
    position: usize,
    f: &SupportFunc,
    executors: &Executors,
) -> Element {
    match f {
        SupportFunc::Add => calc_func::add(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Sub => calc_func::sub(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Mul => calc_func::mul(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Div => calc_func::div(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Mod => calc_func::mod_(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Abs => calc_func::abs(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Ceil => calc_func::ceil(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Floor => calc_func::floor(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Round => calc_func::round(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Trunc => calc_func::trunc(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Sign => calc_func::sign(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Sqrt => calc_func::sqrt(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Exp => calc_func::exp(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Ln => calc_func::ln(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Log10 => calc_func::log10(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::ToLong => calc_func::to_long(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::ToDouble => calc_func::to_double(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Pow => calc_func::pow(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Greatest => calc_func::greatest(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::Least => calc_func::least(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::DimHas => calc_func::dim_has(sub_data_ptr, id, v_ptr, record, position, executors),
        SupportFunc::DimGet => calc_func::dim_get(sub_data_ptr, id, v_ptr, record, position, executors),
        _ => Element::Long(0),
    }
}

pub(crate) fn create_executor(
    record_id_array: &Vec<u16>,
    entities: &Vec<ExprEntity>,
) -> Result<Vec<Executor>, ParseSqlError> {
    let mut executors = vec![];
    for entity in entities {
        let rs = conv_as_executor(entity, record_id_array);
        if let Ok(executor) = rs {
            executors.push(executor);
        } else {
            return Err(rs.err().unwrap());
        }
    }
    Ok(executors)
}

fn conv_as_executor(
    entity: &ExprEntity,
    record_id_array: &Vec<u16>,
) -> Result<Executor, ParseSqlError> {
    let fetcher = match entity {
        ExprEntity::Val(val) => match val {
            ValType::Bool(b) => Executor::ConstLong(if *b { 1 } else { 0 }),
            ValType::Int(i) => Executor::ConstLong(*i),
            ValType::Float(f) => Executor::ConstDouble(*f),
        },
        ExprEntity::Field(field_id) => {
            Executor::Fetch(*record_id_array.first().unwrap(), *field_id)
        }
        ExprEntity::FieldWithTab(record_id, field_id) => Executor::Fetch(*record_id, *field_id),
        ExprEntity::Function(func_name, args) => {
            if func_name.starts_with('_') {
                let mut real_func_name = func_name.clone();
                real_func_name.remove(0);
                let opt_func = SupportFunc::from_str(real_func_name.as_str());
                if let Ok(func) = opt_func {
                    let rs = parse_args_fetchers(args, record_id_array);
                    if let Ok(args_fetchers) = rs {
                        Executor::Compute(func, Executors::new(args_fetchers.as_slice()))
                    } else {
                        return Err(rs.err().unwrap());
                    }
                } else {
                    let err_msg = format!("unknown func: {func_name:?}");
                    log::warn!("{err_msg}");
                    return Err(ParseSqlError::new(err_msg));
                }
            } else {
                let err_msg = format!("unknown func: {func_name:?}");
                log::warn!("{err_msg}");
                return Err(ParseSqlError::new(err_msg));
            }
        }
    };
    Ok(fetcher)
}

fn parse_args_fetchers(
    args: &Vec<ExprEntity>,
    record_id_array: &Vec<u16>,
) -> Result<Vec<Executor>, ParseSqlError> {
    let mut args_fetchers = vec![];
    let mut hit_error = false;
    args.iter()
        .map(|arg| conv_as_executor(arg, record_id_array))
        .for_each(|e| {
            if let Ok(fetcher) = e {
                args_fetchers.push(fetcher);
            } else {
                hit_error = true;
            }
        });
    if hit_error {
        Err(ParseSqlError::new(format!(
            "failed to parse args: {args:?}"
        )))
    } else {
        Ok(args_fetchers)
    }
}
