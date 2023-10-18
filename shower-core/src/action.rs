use crate::{
    error::ActionError,
    sql::{ExprEntity, ParsedSql},
};
use std::str::FromStr;
use strum_macros::EnumString;

pub(crate) fn gen_action(parsed_sql: &ParsedSql) -> Result<Vec<Action>, ActionError> {
    let mut actions = vec![];
    let tab_id_array = parsed_sql.tables();
    let rs = create_filter(parsed_sql.filters());
    if let Ok(filter) = rs {
        actions.push(Action::FilterAction(filter));
    } else {
        return Err(ActionError::new_string(format!(
            "failed to parse filter: {}",
            rs.err().unwrap()
        )));
    }
    let rs = create_executor(tab_id_array.clone(), parsed_sql.fields());
    if let Ok(executors) = rs {
        actions.push(Action::ExecuteAction(executors));
    } else {
        return Err(ActionError::new_string(format!(
            "failed to parse executors: {}",
            rs.err().unwrap()
        )));
    }
    Ok(actions)
}

fn create_filter(entities: Vec<ExprEntity>) -> Result<Filter, ActionError> {
    let mut filters = vec![];
    for entity in entities {
        filters.insert(0, Filter::Original(entity));
    }
    loop {
        let len = filters.len();
        if filters.is_empty() {
            return Err(ActionError::new("No filters found"));
        } else if len == 1 {
            break;
        } else if len >= 3 {
            let v1 = filters.remove(0);
            let v2 = filters.remove(0);
            let op = filters.remove(0);
            let rs = match (v1.clone(), v2.clone(), op.clone()) {
                (Filter::Original(_), Filter::Original(_), Filter::Original(_)) => {
                    Ok(Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()]))
                }
                (Filter::Original(_), Filter::Mixed(_), Filter::Original(_)) => {
                    Ok(Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()]))
                }
                (Filter::Mixed(_), Filter::Original(_), Filter::Original(_)) => {
                    Ok(Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()]))
                }
                (Filter::Mixed(_), Filter::Mixed(_), Filter::Original(_)) => {
                    Ok(Filter::Mixed(vec![v1.clone(), v2.clone(), op.clone()]))
                }
                _ => Err(ActionError::new_string(format!(
                    "cannot hit this case: {:?}",
                    (v1.clone(), v2.clone(), op.clone())
                ))),
            };
            if let Ok(new_filter) = rs {
                filters.insert(0, new_filter);
            } else {
                return Err(rs.err().unwrap());
            }
        } else {
            return Err(ActionError::new("cannot create filter, less than 3"));
        }
    }
    Ok(filters.get(0).cloned().unwrap())
}

fn create_executor(
    tab_id_array: Vec<u16>,
    entities: Vec<ExprEntity>,
) -> Result<Vec<Executor>, ActionError> {
    let mut executors = vec![];
    for entity in entities {
        let rs = conv_as_executor(entity, &tab_id_array);
        if let Ok(executor) = rs {
            executors.push(executor);
        } else {
            return Err(rs.err().unwrap());
        }
    }
    Ok(executors)
}

fn conv_as_executor(entity: ExprEntity, tab_id_array: &Vec<u16>) -> Result<Executor, ActionError> {
    let fetcher = match entity {
        ExprEntity::Field(field_id) => {
            Executor::Fetch(tab_id_array.get(0).cloned().unwrap(), field_id)
        }
        ExprEntity::FieldWithTab(tab_id, field_id) => Executor::Fetch(tab_id, field_id),
        ExprEntity::Function(func_name, args) => {
            if func_name.starts_with("_") {
                let mut real_func_name = func_name.clone();
                real_func_name.remove(0);
                let opt_func = Func::from_str(real_func_name.as_str());
                if let Ok(func) = opt_func {
                    let rs = parse_args_fetchers(args, tab_id_array);
                    if let Ok(args_fetchers) = rs {
                        Executor::Compute(func, args_fetchers)
                    } else {
                        return Err(rs.err().unwrap());
                    }
                } else {
                    return Err(ActionError::new_string(format!(
                        "unknown func: {:?}",
                        func_name
                    )));
                }
            } else {
                return Err(ActionError::new_string(format!(
                    "unknown func: {:?}",
                    func_name
                )));
            }
        }
        _ => {
            return Err(ActionError::new_string(format!(
                "cannot hit this case: {:?}",
                entity
            )));
        }
    };
    Ok(fetcher)
}

fn parse_args_fetchers(
    args: Vec<ExprEntity>,
    tab_id_array: &Vec<u16>,
) -> Result<Vec<Executor>, ActionError> {
    let mut args_fetchers = vec![];
    let mut hit_error = false;
    args.iter()
        .map(|arg| conv_as_executor(arg.clone(), tab_id_array))
        .for_each(|e| {
            if let Ok(fetcher) = e {
                args_fetchers.push(fetcher);
            } else {
                hit_error = true;
            }
        });
    if hit_error {
        Err(ActionError::new_string(format!(
            "failed to parse args: {:?}",
            args
        )))
    } else {
        Ok(args_fetchers)
    }
}

/*
pub(crate) fn invoke(actions: &Vec<Action>) {
    for action in actions {
        match action {
            Action::ExecuteAction(executors) => {
                for executor in executors {
                    call_executor(executor);
                }
            }
            Action::FilterAction(filter) => {
                call_filter(filter);
            }
        }
    }
}
 */

#[derive(Debug, Clone)]
pub(crate) enum Action {
    FilterAction(Filter),
    ExecuteAction(Vec<Executor>),
}
#[derive(Debug, Clone)]
pub(crate) enum Filter {
    Original(ExprEntity),
    Mixed(Vec<Filter>),
}
#[derive(Debug, Clone)]
pub(crate) enum Executor {
    Fetch(u16, u16),
    Compute(Func, Vec<Executor>),
}
#[derive(Debug, Clone, EnumString)]
pub(crate) enum Func {
    #[strum(ascii_case_insensitive)]
    Add,
    #[strum(ascii_case_insensitive)]
    Sub,
}
