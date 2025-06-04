use crate::{
    error::ParseSqlError,
    func::{Executor, Executors, Func},
    sql::base::{ExprEntity, ValType},
};
use std::str::FromStr;

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
                let opt_func = Func::from_str(real_func_name.as_str());
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
